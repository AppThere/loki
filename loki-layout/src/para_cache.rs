// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 AppThere Loki contributors

//! Paragraph shaping cache.
//!
//! Shaping (Parley line-breaking + glyph positioning) is the dominant cost of
//! [`crate::layout_document`], and every keystroke currently re-lays-out the
//! whole document. Because a single edit changes exactly one paragraph, the
//! other `N − 1` paragraphs produce byte-identical layout output. This cache
//! memoises [`ParagraphLayout`] keyed by a hash of every input that affects the
//! output, so an unchanged paragraph is served by a cheap clone instead of a
//! full re-shape — turning per-keystroke cost from `O(n)` shapes into `O(1)`
//! shapes plus `O(n)` clones.
//!
//! The cache lives inside [`crate::FontResources`] (already the shared,
//! per-session layout context threaded into every shaping call), so no public
//! API changes and the cache is automatically shared across layout passes.

use std::collections::HashMap;
use std::collections::hash_map::DefaultHasher;
use std::hash::{Hash, Hasher};
use std::sync::Arc;

use crate::items::{GlyphEntry, PositionedItem};
use crate::para::{ParagraphLayout, ResolvedParaProps, StyleSpan};

/// Maximum live entries before the cache rotates (evicting the older
/// generation). Each entry retains a `ParagraphLayout` (glyph runs, byte-index
/// maps, and a Parley `Layout`), a few KB to tens of KB, so this is a real
/// memory ceiling. 2048 (×2 generations) comfortably covers a ~80-page document
/// — well beyond typical use — while the per-document [`ParaCache::clear`] on
/// load prevents accumulation across documents.
const CACHE_CAP: usize = 2048;

/// Two-generation paragraph-layout cache (approximate LRU).
///
/// Entries hit in the older generation are promoted to the current one, so the
/// working set survives rotation; everything not touched within one rotation is
/// dropped. This bounds memory to roughly `2 × CACHE_CAP` entries without the
/// per-entry bookkeeping of a true LRU.
///
/// # Why `Arc` (Spec 09 S9-1)
///
/// Entries are `Arc<ParagraphLayout>` so the cache and the **page editing
/// index** share one allocation instead of holding two deep copies. Before this,
/// a hit cloned the whole layout out of the cache and `place_paragraph_layout`
/// cloned it a second time for `PageParagraphData` — measured at 39.3 B/char of
/// per-placement editing residency (`docs/spikes/S09.0-layout-residency-census.md`
/// §10c). Handing out the `Arc` makes a hit a refcount bump.
///
/// **Consequence for eviction:** the cache is now an owner of record for glyph
/// data, so dropping `editing_data` alone frees nothing while an entry survives.
/// S9-4 and S9-5 must release both owners, and per R9-07 this cache is bounded
/// by entry count rather than bytes — `CACHE_CAP` binds only past ~1000 pages,
/// so across the realistic range it holds every paragraph.
#[derive(Default)]
pub(crate) struct ParaCache {
    current: HashMap<u64, Arc<ParagraphLayout>>,
    previous: HashMap<u64, Arc<ParagraphLayout>>,
}

impl ParaCache {
    /// Returns the cached layout for `key`, if present — a refcount bump, not a
    /// copy. A hit in the older generation is promoted so it is not lost at the
    /// next rotation.
    pub(crate) fn get(&mut self, key: u64) -> Option<Arc<ParagraphLayout>> {
        if let Some(v) = self.current.get(&key) {
            return Some(Arc::clone(v));
        }
        if let Some(v) = self.previous.remove(&key) {
            let out = Arc::clone(&v);
            self.current.insert(key, v);
            return Some(out);
        }
        None
    }

    /// Inserts `value` under `key`, rotating generations when the current one is
    /// full.
    pub(crate) fn put(&mut self, key: u64, value: Arc<ParagraphLayout>) {
        if self.current.len() >= CACHE_CAP {
            self.previous = std::mem::take(&mut self.current);
        }
        self.current.insert(key, value);
    }

    /// Drops every cached entry, freeing the retained `ParagraphLayout`s.
    ///
    /// Called when a new document is loaded so the cache does not retain the
    /// previous document's paragraph layouts.
    pub(crate) fn clear(&mut self) {
        self.current.clear();
        self.previous.clear();
    }

    /// Number of distinct entries currently resident (both generations). Used by
    /// tests to assert hit/miss behaviour.
    #[cfg(test)]
    pub(crate) fn len(&self) -> usize {
        self.current.len() + self.previous.len()
    }

    /// `(entries, approximate heap bytes)` currently resident, both
    /// generations — the usage-audit §15/A1 instrumentation. The byte figure
    /// is a **floor**: it walks the glyph vectors, line boundaries, and index
    /// maps (the dominant owned allocations) but not the opaque retained
    /// Parley `Layout` or shared `Arc` payloads (font bytes, images), which
    /// would be multiply counted. Entries shared with the page editing index
    /// via `Arc` are counted once, here, as cache residency.
    pub(crate) fn stats(&self) -> (usize, usize) {
        let bytes = self
            .current
            .values()
            .chain(self.previous.values())
            .map(|l| layout_bytes(l))
            .sum();
        (self.current.len() + self.previous.len(), bytes)
    }
}

/// Approximate owned heap bytes of one cached [`ParagraphLayout`] — see
/// [`ParaCache::stats`] for what is (and deliberately is not) counted.
fn layout_bytes(l: &ParagraphLayout) -> usize {
    let items: usize = l.items.capacity() * std::mem::size_of::<PositionedItem>()
        + l.items.iter().map(item_bytes).sum::<usize>();
    items
        + l.line_boundaries.capacity() * std::mem::size_of::<(f32, f32)>()
        + l.orig_to_clean.approx_heap_bytes()
        + l.clean_to_orig.approx_heap_bytes()
}

/// The glyph-vector bytes of one item (nested groups walked); fixed-size
/// rect/rule items own no counted heap.
fn item_bytes(item: &PositionedItem) -> usize {
    match item {
        PositionedItem::GlyphRun(r) => r.glyphs.capacity() * std::mem::size_of::<GlyphEntry>(),
        PositionedItem::ClippedGroup { items, .. } | PositionedItem::RotatedGroup { items, .. } => {
            items.capacity() * std::mem::size_of::<PositionedItem>()
                + items.iter().map(item_bytes).sum::<usize>()
        }
        _ => 0,
    }
}

/// `fmt::Write` sink that feeds formatted bytes straight into a [`Hasher`], so
/// Debug-formatting the style structs for [`para_key`] costs no heap `String`.
/// This runs once per paragraph per layout pass — including cache *hits* — so
/// the allocation it avoids was paid on every keystroke for every paragraph.
struct HashWriter<'a, H: Hasher>(&'a mut H);

impl<H: Hasher> std::fmt::Write for HashWriter<'_, H> {
    fn write_str(&mut self, s: &str) -> std::fmt::Result {
        self.0.write(s.as_bytes());
        Ok(())
    }
}

/// Computes the cache key for one [`crate::para::layout_paragraph`] call.
///
/// CACHE KEY INVARIANT: this must incorporate *every* input that can change the
/// produced [`ParagraphLayout`]. `style_spans` and `para_props` are folded in
/// via their `Debug` representation **on purpose** — the derived `Debug`
/// includes every field (and Rust's `f32` `Debug` is round-trip exact), so
/// adding a field to [`StyleSpan`] or [`ResolvedParaProps`] is covered
/// automatically instead of silently producing stale cache hits.
pub(crate) fn para_key(
    text_content: &str,
    style_spans: &[StyleSpan],
    para_props: &ResolvedParaProps,
    available_width: f32,
    display_scale: f32,
    preserve_for_editing: bool,
    spell_generation: u64,
) -> u64 {
    use std::fmt::Write as _;

    let mut hasher = DefaultHasher::new();
    text_content.hash(&mut hasher);
    available_width.to_bits().hash(&mut hasher);
    display_scale.to_bits().hash(&mut hasher);
    preserve_for_editing.hash(&mut hasher);
    // 0 = no spell checking; non-zero generations distinguish dictionary /
    // personal-word-list states that the paragraph text alone cannot express.
    spell_generation.hash(&mut hasher);

    // Debug-format the style structs so the key tracks struct evolution without
    // manual per-field maintenance, streaming the bytes into the hasher.
    let _ = write!(HashWriter(&mut hasher), "{style_spans:?}|{para_props:?}");

    hasher.finish()
}

#[cfg(test)]
#[path = "para_cache_tests.rs"]
mod tests;
