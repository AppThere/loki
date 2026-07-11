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
    /// The shared, thread-safe checker queried during layout.
    pub checker: std::sync::Arc<loki_spell::SpellChecker>,
    /// Monotonic generation; bump on any change the text alone cannot express.
    pub generation: u64,
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
