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

use crate::para::{ParagraphLayout, ResolvedParaProps, StyleSpan};

#[path = "para_cache_bytes.rs"]
mod bytes;
use bytes::layout_bytes;

/// Maximum live entries before the cache rotates (evicting the older
/// generation). Retained as a secondary guard for documents of very many very
/// small paragraphs, where [`GENERATION_BYTE_CAP`] would not bind.
const CACHE_CAP: usize = 2048;

/// Retained bytes (per generation) before the cache rotates — the bound that
/// actually binds while typing.
///
/// # Why an entry count was the wrong bound
///
/// The key is a hash of the paragraph *text*, so every keystroke mints a new
/// entry and the paragraph's superseded versions stay resident. Worse, each
/// successive version of a paragraph being typed into is **longer** than the
/// last, so the retained bytes of a typing burst grow with the square of its
/// length. Measured headlessly (40 stable paragraphs + one edited, the fixture
/// `a_typing_burst_stays_within_the_byte_bound` now asserts against):
/// 100 keystrokes retained 150 KiB, 600 retained 3.2 MiB — 6× the keystrokes
/// for 21.7× the bytes, against a live working set of 41 entries. `CACHE_CAP`
/// could not intervene until 2048 entries, by which point the garbage is
/// thousands of times the working set.
///
/// Rotation is the eviction: the superseded versions are never looked up again,
/// so they are exactly what the older generation drops, while the live working
/// set is promoted back on its next hit.
///
/// # Why 8 MiB specifically
///
/// It is where [`CACHE_CAP`] already put the cliff, so large documents behave as
/// they did. Measured at ~3.7 KiB per paragraph of ordinary prose (glyphs,
/// index maps and the retained Parley layout — see `para_cache_bytes`), 2048
/// entries *is* about 7.6 MiB. A document big enough to be evicted under this
/// bound was already being evicted by the entry cap; what changes is that a
/// **typing burst** now hits a ceiling too, instead of accumulating superseded
/// versions until it reaches an entry count it may never reach.
const GENERATION_BYTE_CAP: usize = 8 * 1024 * 1024;

/// One cached layout and the byte figure it contributed when it was inserted.
///
/// The size is stored rather than recomputed so that promotion, eviction and
/// [`ParaCache::stats`] are all O(1) — [`layout_bytes`] walks every glyph run,
/// and it used to run once per resident entry on every `stats()` call.
struct Entry {
    layout: Arc<ParagraphLayout>,
    bytes: usize,
}

/// Two-generation paragraph-layout cache (approximate LRU), bounded by retained
/// bytes and by entry count.
///
/// Entries hit in the older generation are promoted to the current one, so the
/// working set survives rotation; everything not touched within one rotation is
/// dropped. This bounds memory without the per-entry bookkeeping of a true LRU.
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
/// **Consequence for eviction:** the cache is an owner of record for glyph data,
/// so dropping `editing_data` alone frees nothing while an entry survives. S9-4
/// and S9-5 must release both owners.
#[derive(Default)]
pub(crate) struct ParaCache {
    current: HashMap<u64, Entry>,
    previous: HashMap<u64, Entry>,
    /// Sum of `Entry::bytes` over `current`. Maintained incrementally; pinned to
    /// the walked total by `maintained_byte_total_matches_a_full_walk`.
    current_bytes: usize,
    /// Sum of `Entry::bytes` over `previous`.
    previous_bytes: usize,
    /// Lifetime count of insertions — i.e. of paragraphs that had to be shaped
    /// because the cache could not serve them.
    ///
    /// The instrument for "is the bound being met by evicting garbage, or by
    /// starving the working set?". End-state residency cannot answer that: a
    /// cache that drops everything on rotation is refilled by the very next
    /// layout pass, so it *looks* populated whenever it is inspected. Only the
    /// re-shape count over the whole burst separates the two.
    #[cfg(test)]
    pub(crate) misses: usize,
}

impl ParaCache {
    /// Returns the cached layout for `key`, if present — a refcount bump, not a
    /// copy. A hit in the older generation is promoted so it is not lost at the
    /// next rotation.
    pub(crate) fn get(&mut self, key: u64) -> Option<Arc<ParagraphLayout>> {
        if let Some(e) = self.current.get(&key) {
            return Some(Arc::clone(&e.layout));
        }
        if let Some(e) = self.previous.remove(&key) {
            // Promotion moves bytes between generations without adding any, so
            // it deliberately does *not* test the cap: rotating here could evict
            // entries promoted earlier in the same layout pass, which is the
            // thrash the pass is trying to avoid.
            self.previous_bytes = self.previous_bytes.saturating_sub(e.bytes);
            self.current_bytes = self.current_bytes.saturating_add(e.bytes);
            let out = Arc::clone(&e.layout);
            self.current.insert(key, e);
            return Some(out);
        }
        None
    }

    /// Inserts `value` under `key`, rotating generations when the current one is
    /// full by either bound.
    pub(crate) fn put(&mut self, key: u64, value: Arc<ParagraphLayout>) {
        #[cfg(test)]
        {
            self.misses += 1;
        }
        let bytes = layout_bytes(&value);
        if self.current.len() >= CACHE_CAP
            || self.current_bytes.saturating_add(bytes) > GENERATION_BYTE_CAP
        {
            self.rotate();
        }
        self.current_bytes = self.current_bytes.saturating_add(bytes);
        // `put` only follows a `get` miss, so a replacement should be
        // unreachable — but crediting the old entry's bytes back keeps
        // `current_bytes` equal to the walked sum unconditionally, which is what
        // the bound is trusted to mean.
        if let Some(old) = self.current.insert(
            key,
            Entry {
                layout: value,
                bytes,
            },
        ) {
            self.current_bytes = self.current_bytes.saturating_sub(old.bytes);
        }
    }

    /// Retires the current generation, dropping whatever the previous one still
    /// held. The live working set is not lost: its next hit promotes it back.
    fn rotate(&mut self) {
        self.previous = std::mem::take(&mut self.current);
        self.previous_bytes = std::mem::replace(&mut self.current_bytes, 0);
    }

    /// Drops every cached entry, freeing the retained `ParagraphLayout`s.
    ///
    /// Called when a document is loaded or closed so the cache does not retain
    /// a document no editor is showing.
    pub(crate) fn clear(&mut self) {
        self.current.clear();
        self.previous.clear();
        self.current_bytes = 0;
        self.previous_bytes = 0;
    }

    /// Number of distinct entries currently resident (both generations). Used by
    /// tests to assert hit/miss behaviour.
    #[cfg(test)]
    pub(crate) fn len(&self) -> usize {
        self.current.len() + self.previous.len()
    }

    /// `(entries, approximate heap bytes)` currently resident, both
    /// generations — the usage-audit §15/A1 instrumentation, and the quantity
    /// [`GENERATION_BYTE_CAP`] bounds.
    ///
    /// The byte figure is still a **floor**, but a much closer one than it was:
    /// it now includes the retained Parley layout (~26 B/char, previously
    /// omitted and larger than everything else combined). What remains
    /// uncounted is parley's own capacity slack, which it exposes no way to
    /// read, and the shared `Arc` payloads — font bytes and images — which
    /// would be multiply counted. Entries shared with the page editing index
    /// via `Arc` are counted once, here, as cache residency. See
    /// `para_cache_bytes`.
    pub(crate) fn stats(&self) -> (usize, usize) {
        (
            self.current.len() + self.previous.len(),
            self.current_bytes.saturating_add(self.previous_bytes),
        )
    }

    /// Recomputes the retained total by walking every entry — the independent
    /// derivation the maintained counters are pinned against.
    #[cfg(test)]
    pub(crate) fn walked_byte_total(&self) -> usize {
        self.current
            .values()
            .chain(self.previous.values())
            .map(|e| layout_bytes(&e.layout))
            .sum()
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
