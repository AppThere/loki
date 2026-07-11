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
#[derive(Default)]
pub(crate) struct ParaCache {
    current: HashMap<u64, ParagraphLayout>,
    previous: HashMap<u64, ParagraphLayout>,
}

impl ParaCache {
    /// Returns a clone of the cached layout for `key`, if present. A hit in the
    /// older generation is promoted so it is not lost at the next rotation.
    pub(crate) fn get(&mut self, key: u64) -> Option<ParagraphLayout> {
        if let Some(v) = self.current.get(&key) {
            return Some(v.clone());
        }
        if let Some(v) = self.previous.remove(&key) {
            let out = v.clone();
            self.current.insert(key, v);
            return Some(out);
        }
        None
    }

    /// Inserts `value` under `key`, rotating generations when the current one is
    /// full.
    pub(crate) fn put(&mut self, key: u64, value: ParagraphLayout) {
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
mod tests {
    use crate::color::LayoutColor;
    use crate::font::FontResources;
    use crate::para::{ResolvedParaProps, StyleSpan, layout_paragraph};

    fn resources() -> FontResources {
        let mut r = FontResources::new();
        for p in [
            "/usr/share/fonts/truetype/liberation/LiberationSans-Regular.ttf",
            "/usr/share/fonts/truetype/liberation/LiberationSans-Bold.ttf",
        ] {
            if let Ok(data) = std::fs::read(p) {
                r.register_font(data);
            }
        }
        r
    }

    fn span(text: &str) -> StyleSpan {
        StyleSpan {
            range: 0..text.len(),
            font_name: None,
            font_size: 12.0,
            bold: false,
            weight: 400,
            italic: false,
            color: LayoutColor::BLACK,
            underline: None,
            strikethrough: None,
            line_height: None,
            vertical_align: None,
            highlight_color: None,
            letter_spacing: None,
            font_variant: None,
            word_spacing: None,
            shadow: false,
            link_url: None,
            math: None,
            scale: None,
            kerning: None,
            baseline_shift: None,
        }
    }

    fn lay(r: &mut FontResources, text: &str, spans: &[StyleSpan], width: f32) {
        let _ = layout_paragraph(
            r,
            text,
            spans,
            &ResolvedParaProps::default(),
            width,
            1.0,
            true,
        );
    }

    #[test]
    fn identical_inputs_hit_and_match() {
        let mut r = resources();
        let text = "Hello cache world";
        let spans = [span(text)];

        let first = layout_paragraph(
            &mut r,
            text,
            &spans,
            &ResolvedParaProps::default(),
            400.0,
            1.0,
            true,
        );
        assert_eq!(
            r.para_cache.len(),
            1,
            "first call should populate the cache"
        );

        let second = layout_paragraph(
            &mut r,
            text,
            &spans,
            &ResolvedParaProps::default(),
            400.0,
            1.0,
            true,
        );
        // Identical inputs must be a hit (no new entry) and reproduce the layout.
        assert_eq!(
            r.para_cache.len(),
            1,
            "identical call should hit, not insert"
        );
        assert_eq!(first.height, second.height);
        assert_eq!(first.width, second.width);
        assert_eq!(first.items.len(), second.items.len());
    }

    #[test]
    fn changed_inputs_are_misses() {
        let mut r = resources();
        let base = "alpha";

        lay(&mut r, base, &[span(base)], 400.0);
        assert_eq!(r.para_cache.len(), 1);

        // Different text.
        lay(&mut r, "bravo", &[span("bravo")], 400.0);
        assert_eq!(r.para_cache.len(), 2, "different text must miss");

        // Different width, same text/spans.
        lay(&mut r, base, &[span(base)], 200.0);
        assert_eq!(r.para_cache.len(), 3, "different width must miss");

        // Different char property (bold) on the same text.
        let mut bold = span(base);
        bold.bold = true;
        lay(&mut r, base, &[bold], 400.0);
        assert_eq!(r.para_cache.len(), 4, "different style span must miss");
    }

    #[test]
    fn clear_drops_all_entries() {
        let mut r = resources();
        lay(&mut r, "one", &[span("one")], 400.0);
        lay(&mut r, "two", &[span("two")], 400.0);
        assert_eq!(r.para_cache.len(), 2);

        r.clear_paragraph_cache();
        assert_eq!(r.para_cache.len(), 0, "clear should drop every entry");

        // A subsequent layout repopulates from scratch (miss, not stale hit).
        lay(&mut r, "one", &[span("one")], 400.0);
        assert_eq!(r.para_cache.len(), 1);
    }

    #[test]
    fn preserve_flag_is_part_of_key() {
        let mut r = resources();
        let text = "preserve flag";
        let spans = [span(text)];
        let props = ResolvedParaProps::default();

        let _ = layout_paragraph(&mut r, text, &spans, &props, 400.0, 1.0, true);
        let _ = layout_paragraph(&mut r, text, &spans, &props, 400.0, 1.0, false);
        assert_eq!(
            r.para_cache.len(),
            2,
            "preserve_for_editing must distinguish cache entries"
        );
    }
}
