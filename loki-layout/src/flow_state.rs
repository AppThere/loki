// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 AppThere Loki contributors

//! The flow engine's mutable per-run state, split out of `flow.rs` at the
//! 300-line ceiling.
//!
//! `flow.rs` is the module root: it declares twenty submodules and re-exports
//! the engine's public surface, so it has no room left for the struct those
//! submodules all mutate. The struct lives here; its methods stay with the
//! code that uses them (`flow_run.rs`, and `flow.rs` for the checkpoint
//! snapshot).

use std::collections::HashMap;

use loki_doc_model::StyleCatalog;
use loki_doc_model::style::list_style::ListId;

use crate::LayoutOptions;
use crate::font::FontResources;
use crate::geometry::{LayoutInsets, LayoutSize};
use crate::incremental::PageStart;
use crate::items::PositionedItem;
use crate::mode::LayoutMode;
use crate::resolve::CollectedNote;
use crate::result::{LayoutPage, PageParagraphData};

use super::{LayoutWarning, editing, float_impl, line_numbers, para_between};

pub(crate) struct FlowState<'a> {
    pub(super) resources: &'a mut FontResources,
    pub(super) catalog: &'a StyleCatalog,
    pub(super) mode: &'a LayoutMode,
    pub(super) display_scale: f32,
    /// Layout options forwarded from the [`layout_document`] caller.
    pub(super) options: &'a LayoutOptions,
    /// Current y within the current page content area (or canvas).
    pub(super) cursor_y: f32,
    pub(super) content_width: f32,
    /// Items accumulating in the current page (or entire canvas for continuous).
    pub(super) current_items: Vec<PositionedItem>,
    /// Completed pages (paginated mode only).
    pub(super) pages: Vec<LayoutPage>,
    pub(super) page_size: LayoutSize,
    pub(super) margins: LayoutInsets,
    /// Height of the content area within a page (page_height − v_margins).
    pub(super) page_content_height: f32,
    /// 1-indexed current page number.
    pub(super) page_number: usize,
    /// Accumulated warnings.
    pub(super) warnings: Vec<LayoutWarning>,
    /// Accumulated horizontal indentation in points.
    pub(super) current_indent: f32,
    /// Per-list counters: `ListId` → per-level counters (`0` = uninitialised).
    pub(super) list_counters: HashMap<ListId, [u32; 9]>,
    /// `ListId` of the most recently placed list item (detects list changes).
    pub(super) prev_list_id: Option<ListId>,
    /// Footnote/endnote counter for the section (bumped by `walk_inlines`).
    pub(super) note_counter: u32,
    /// Footnotes whose reference is on the **current page**, laid out at its
    /// foot by `finish_page` (their height reserved from `page_content_height`
    /// so body content stops above the band — per-page, matching Word).
    pub(super) pending_footnotes: Vec<CollectedNote>,
    /// Endnotes, held for the section-end flush (`flow_footnotes`) rather than
    /// the per-page footnote band.
    pub(super) pending_endnotes: Vec<CollectedNote>,
    /// Points reserved at the foot of the **current page** for the footnotes
    /// collected so far (separator band + each note's measured height). Shrinks
    /// [`content_bottom`](Self::content_bottom) so body content stops above the
    /// band; reset to `0` at each page boundary (`finish_page`).
    pub(super) footnote_reserved: f32,
    /// Re-entrancy guard: `true` while `finish_page` is laying out the footnote
    /// band, so a nested page flush during that work does not recurse.
    pub(super) rendering_footnotes: bool,
    /// Paragraph metadata for the current page (block index, layout, origin).
    pub(super) current_paragraphs: Vec<PageParagraphData>,
    /// Clean-page-top checkpoints for incremental relayout (top-level only).
    pub(super) checkpoints: Vec<PageStart>,
    /// Number of text columns (`1` = single); when `> 1`,
    /// [`content_width`](Self::content_width) is the *current* column's width.
    pub(super) columns: u8,
    /// Per-column widths in points (length `columns`; may be unequal). Column
    /// x-offsets are the running sum of preceding widths plus `column_gap` per
    /// gap. `content_width` tracks the current column's entry.
    pub(super) column_widths: Vec<f32>,
    /// Gap between adjacent columns in points (only when `columns > 1`).
    pub(super) column_gap: f32,
    /// Whether to draw a separator line between columns.
    pub(super) column_separator: bool,
    /// 0-based index of the column currently being filled.
    pub(super) col_index: u8,
    /// Content-area y where the current column band begins (`0` normally; mid-page
    /// for a `continuous` section break that starts a band below the previous one).
    pub(super) column_top_y: f32,
    /// First `current_items` index of the current column (shifted at finish).
    pub(super) column_item_start: usize,
    /// First `current_paragraphs` index of the column (parallel to above).
    pub(super) column_para_start: usize,
    /// Document comments, looked up by id for the gutter panel; empty in nested flows.
    pub(super) comments: &'a [loki_doc_model::content::annotation::Comment],
    /// Comment anchors (`id`, content-local `y`) on the current page, consumed by
    /// [`finish_page`] for the gutter comment panel.
    pub(super) pending_comment_anchors: Vec<(String, f32)>,
    /// Break over-long words to the width (`overflow-wrap: anywhere`); set
    /// while flowing table-cell content so words wrap to the column width.
    pub(super) break_long_words: bool,
    /// A float taller than its anchoring paragraph whose remaining extent the
    /// following paragraphs keep wrapping beside; cleared on page boundaries.
    pub(super) active_float: Option<float_impl::ActiveFloat>,
    /// Editing-path context for nested content (see [`editing::NestedEditing`]).
    pub(super) nested_editing: Option<editing::NestedEditing>,
    /// Between-border override for the paragraph about to flow (gap #26).
    pub(super) staged_between: Option<para_between::BetweenOverride>,
    /// Newest block observed to start a fresh page, with its pre-block resume
    /// snapshot — the last-page balancing seed (`flow_balance`; multi-column
    /// flows only). `None`d when a block spans several page advances.
    pub(super) tail_candidate: Option<PageStart>,
    /// Table-region character defaults for the cell currently flowing (4a.3);
    /// merged under the paragraph chain by `flatten_paragraph_with_base`.
    pub(super) cell_char_defaults: Option<loki_doc_model::style::props::char_props::CharProps>,
    /// Active margin line-numbering state for the section (`w:lnNumType`), or
    /// `None` when the section has no line numbering (the common case).
    pub(super) line_num: Option<line_numbers::LineNumberState>,
    /// Space *already contributed* below the cursor by the preceding block's
    /// `space_after`, so the next block's `space_before` can be collapsed
    /// against it rather than added to it — see
    /// [`advance_space_before`](FlowState::advance_space_before).
    ///
    /// Reset to `0` at a [`BreakCause::Flow`] boundary: collapsing across one
    /// would let a paragraph's `space_before` be swallowed by a `space_after`
    /// that is now on the previous page, pulling its first line up against the
    /// top margin. **Kept** across a [`BreakCause::Forced`] one, where Word
    /// does collapse — see that variant.
    pub(super) last_space_after: f32,
    /// Set when the flow arrives at a fresh page or column via a
    /// [`BreakCause::Flow`] break, so the next block's `space_before` is
    /// dropped rather than applied.
    ///
    /// Not the same as "the cursor is at the top of the band": Word applies
    /// `space_before` on the document's very first paragraph (measured: 24 pt
    /// requested, 24 pt applied) and drops it after a flow break (measured:
    /// 36 pt requested, 0 pt applied). Only the arrival route separates those
    /// two, which a cursor-position test cannot see.
    pub(super) suppress_space_before: bool,
}

/// Why a page or column ended. Word treats a paragraph's `space_before` at the
/// top of the new page differently depending on how the flow got there, so the
/// cause has to travel with the break rather than be inferred at the far end.
///
/// Measured against Word 16.0 (`w:before` = 36 pt on the first paragraph of the
/// new page, PDF export, first-baseline against the same document with
/// `w:before` = 0):
///
/// | how the page ended                | applied |
/// |-----------------------------------|---------|
/// | ran out of room (natural overflow)| 0 pt    |
/// | `<w:br w:type="page"/>` run       | 0 pt    |
/// | `w:pageBreakBefore` paragraph     | 36 pt   |
/// | `nextPage` section break          | 36 pt   |
///
/// and with 20 pt of `space_after` on the preceding paragraph, the two
/// *forced* rows fall to 15.96 pt — i.e. they collapse against it by the
/// ordinary `max(after, before)` rule rather than being applied whole. So the
/// distinction is exactly Flow-versus-Forced, not page-break-versus-section.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum BreakCause {
    /// The column or page ran out of vertical room, or a `<w:br w:type="page"/>`
    /// run asked for a new page. The next block's `space_before` is dropped.
    Flow,
    /// A `w:pageBreakBefore` paragraph asked for the break. The next block's
    /// `space_before` survives, collapsed against the preceding block's
    /// `space_after`.
    ///
    /// A `nextPage` section start behaves identically in Word, but Loki flows
    /// each such section as its own page sequence with a fresh `FlowState`, so
    /// it cannot reach this path — see `TODO(section-space-before-collapse)`
    /// in `flow_group.rs`.
    Forced,
}
