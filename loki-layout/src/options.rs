// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 AppThere Loki contributors

//! Layout-pipeline configuration structs: [`LayoutOptions`] (feature / memory
//! trade-offs), [`SpellState`] (checker + invalidation generation), and
//! [`FieldContext`] (resolved page numbering for field substitution).

/// Options that control the layout pipeline's memory / feature trade-offs.
///
/// Pass to [`crate::layout_document`] or [`crate::flow_section`]. The default
/// (all fields `false` / `None`) is the read-only rendering mode — zero
/// overhead for features the renderer does not need.
#[derive(Debug, Clone, Default)]
pub struct LayoutOptions {
    /// When `true`, the Parley `Layout` object is retained inside each
    /// [`crate::ParagraphLayout`] so that
    /// [`crate::ParagraphLayout::hit_test_point`] and
    /// [`crate::ParagraphLayout::cursor_rect`] can be called afterwards.
    ///
    /// Has a memory cost proportional to document size. Use `false` (the
    /// default) for read-only document viewing. Editing sessions pass `true`.
    pub preserve_for_editing: bool,

    /// Optional spell checker. When `Some`, each paragraph's text is checked and
    /// misspelled words emit a [`crate::items::DecorationKind::Spelling`]
    /// squiggle decoration (positioned via the same Parley selection-geometry
    /// mechanism as the highlight underlay). `None` (the default) adds zero
    /// overhead.
    pub spell: Option<SpellState>,

    /// Default tab-stop interval in points, honouring the document's
    /// `DocumentSettings::default_tab_stop_pt` (Word `w:defaultTabStop` / ODF
    /// `style:tab-stop-distance`). `None` (the default) falls back to the
    /// built-in 36 pt (½ inch). Applied when a paragraph runs out of explicit
    /// tab stops.
    pub default_tab_stop_pt: Option<f32>,

    /// Mirror the left/right margins on even (verso) pages (Word
    /// `w:mirrorMargins`, gap #27). Folded in from
    /// `DocumentSettings::mirror_margins` by `layout_document`; the content
    /// width is unchanged (left + right is constant), only the content
    /// area's horizontal position swaps.
    pub mirror_margins: bool,

    /// How tracked changes (`CharProps::revision`) are displayed. The default
    /// [`RevisionDisplay::AllMarkup`] shows insertions underlined and deletions
    /// struck through; [`RevisionDisplay::Final`] / [`RevisionDisplay::Original`]
    /// render the accepted / rejected document **non-destructively** (the
    /// revision marks are untouched — only the view changes), matching Word's
    /// "Show Markup" dropdown. Because the mode changes the flattened text +
    /// spans, the content-addressed paragraph cache keys on it automatically.
    pub revision_display: RevisionDisplay,
}

/// How tracked-change (`CharProps::revision`) runs are rendered — a
/// **non-destructive** view over the document, mirroring Word's Review-tab
/// "Show Markup" dropdown. Switching modes never mutates the revision marks
/// (unlike `accept_revisions` / `reject_revisions`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum RevisionDisplay {
    /// Show insertions (author-coloured, underlined) and deletions
    /// (author-coloured, struck through) inline — Word's "All Markup". Default.
    #[default]
    AllMarkup,
    /// Show the document as if every change were **accepted**: insertions render
    /// as normal text, deletions are hidden — Word's "No Markup" / Final.
    Final,
    /// Show the document as if every change were **rejected**: deletions render
    /// as normal text, insertions are hidden — Word's "Original".
    Original,
}

/// A spell checker plus a cache-invalidation generation, supplied via
/// [`LayoutOptions::spell`].
///
/// The paragraph layout cache is content-addressed; the `generation` folds into
/// the cache key so that changing the active dictionary or the user's
/// personal/ignore words (which the checker reflects but the paragraph text does
/// not) correctly invalidates cached squiggles. The host **must** bump
/// `generation` whenever it swaps the checker or its word lists change; a fresh
/// service starts at `1` (0 is reserved for "no spell checking").
#[derive(Debug, Clone)]
pub struct SpellState {
    /// The shared, thread-safe checker queried during layout. Used for text
    /// with no language tag, and — when [`Self::checkers`] is empty — for all
    /// text (single-dictionary mode).
    pub checker: std::sync::Arc<loki_spell::SpellChecker>,
    /// Per-language checkers keyed by *normalized* BCP-47 tag
    /// (`loki_spell::locale::normalize` — lowercase, `-` separators), resolved
    /// via the locale fallback chain (gap #30). When non-empty, a run tagged
    /// with a language that resolves to **no** entry is *skipped* rather than
    /// checked against the wrong dictionary — so foreign-language runs are not
    /// blanketed in false squiggles (matching LibreOffice). The host should
    /// register the default checker under its own language tag.
    pub checkers: std::collections::HashMap<String, std::sync::Arc<loki_spell::SpellChecker>>,
    /// Monotonic generation; bump on any change the text alone cannot express.
    pub generation: u64,
}

impl SpellState {
    /// The checker for a run tagged `language` (`None` = untagged text).
    ///
    /// Untagged text always uses the default [`Self::checker`]. Tagged text
    /// resolves through the fallback chain (`fr-CA` → `fr-ca`, `fr`) against
    /// [`Self::checkers`]; on a miss it falls back to the default checker in
    /// single-dictionary mode (empty map — the pre-gap-#30 behaviour) and to
    /// `None` (skip checking) in multi-dictionary mode.
    #[must_use]
    pub fn checker_for(
        &self,
        language: Option<&str>,
    ) -> Option<&std::sync::Arc<loki_spell::SpellChecker>> {
        let Some(language) = language else {
            return Some(&self.checker);
        };
        for tag in loki_spell::locale::fallback_chain(language) {
            if let Some(checker) = self.checkers.get(&tag) {
                return Some(checker);
            }
        }
        self.checkers.is_empty().then_some(&self.checker)
    }
}

#[cfg(test)]
mod spell_state_tests {
    use std::sync::Arc;

    use super::SpellState;

    fn state(tags: &[&str]) -> SpellState {
        let checker = Arc::new(loki_spell::SpellChecker::bundled().expect("bundled loads"));
        SpellState {
            checkers: tags
                .iter()
                .map(|t| ((*t).to_string(), Arc::clone(&checker)))
                .collect(),
            checker,
            generation: 1,
        }
    }

    #[test]
    fn untagged_text_uses_the_default_checker() {
        let s = state(&["fr"]);
        assert!(
            s.checker_for(None)
                .is_some_and(|c| Arc::ptr_eq(c, &s.checker))
        );
    }

    #[test]
    fn tagged_text_resolves_through_the_fallback_chain() {
        let s = state(&["fr"]);
        assert!(
            s.checker_for(Some("fr-CA")).is_some(),
            "fr-CA falls back to fr"
        );
        assert!(
            s.checker_for(Some("FR_ca")).is_some(),
            "normalization applies"
        );
    }

    #[test]
    fn unmatched_tag_skips_in_multi_dictionary_mode() {
        let s = state(&["fr"]);
        assert!(
            s.checker_for(Some("de-DE")).is_none(),
            "no de dictionary → skip"
        );
    }

    #[test]
    fn unmatched_tag_falls_back_in_single_dictionary_mode() {
        let s = state(&[]);
        assert!(
            s.checker_for(Some("de-DE"))
                .is_some_and(|c| Arc::ptr_eq(c, &s.checker)),
            "empty map keeps the legacy check-everything behaviour"
        );
    }
}

/// Resolved page numbering for field substitution during layout.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct FieldContext {
    /// 1-based page number of the page being laid out (already offset by any
    /// section restart value).
    pub page_number: u32,
    /// Total page count of the document.
    pub page_count: u32,
    /// Display format for the PAGE field (OOXML `w:pgNumType @w:fmt`).
    /// `None` = decimal.
    pub number_format: Option<loki_doc_model::style::list_style::NumberingScheme>,
}
