// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 AppThere Loki contributors

//! Renderer-agnostic layout engine for the Loki suite.
//!
//! `loki-layout` turns a [`loki_doc_model::Document`] into absolute positions
//! for all content — no GPU dependencies, fully testable without a display.
//!
//! # Layout Modes
//!
//! Three modes are supported via [`LayoutMode`]:
//!
//! - [`LayoutMode::Paginated`]: content broken into fixed-size pages.
//! - [`LayoutMode::Pageless`]: single infinite canvas, document-width content.
//! - [`LayoutMode::Reflow`]: single infinite canvas, caller-supplied width.
//!
//! # Output
//!
//! Layout produces a [`DocumentLayout`] containing [`PositionedItem`]s, each
//! carrying absolute coordinates ready for a renderer such as `loki-vello`.

#![forbid(unsafe_code)]
#![warn(missing_docs)]

pub mod color;
pub mod error;
pub mod flow;
pub mod font;
pub mod geometry;
pub mod incremental;
pub mod items;
mod list_marker;
mod math;
pub mod mode;
pub mod para;
mod para_band;
mod para_cache;
mod para_drop_cap;
mod para_emit;
pub mod resolve;
pub mod result;
mod revision_style;
mod table_shading;
pub use color::LayoutColor;
pub use error::{LayoutError, LayoutResult};
pub use flow::{FlowOutput, LayoutWarning, flow_section};
pub use font::FontResources;
pub use geometry::{LayoutInsets, LayoutPoint, LayoutRect, LayoutSize};
pub use incremental::{
    FlowCheckpoint, PageStart, PaginatedReuse, document_has_notes, relayout_paginated_incremental,
};
pub use items::{
    BorderEdge, BorderStyle, DecorationKind, GlyphEntry, GlyphSynthesis, PositionedBorderRect,
    PositionedDecoration, PositionedGlyphRun, PositionedImage, PositionedItem, PositionedRect,
};
pub use mode::LayoutMode;
pub use para::{
    Affinity, CursorRect, HitTestResult, ParagraphLayout, ResolvedLineHeight, ResolvedParaProps,
    StyleSpan, layout_paragraph,
};
pub use resolve::{
    CollectedImage, CollectedNote, emu_to_pt, flatten_paragraph, pts_to_f32, resolve_char_props,
    resolve_color, resolve_para_props,
};
pub use result::{
    ContinuousLayout, DocumentLayout, LayoutPage, PageEditingData, PageParagraphData,
    PaginatedLayout,
};

/// Minimum table row height in points.
pub const MIN_ROW_HEIGHT: f32 = 0.0;

/// Total width (points) reserved to the right of the page for the comment
/// gutter panel (gap + card width). Hosts widen the scrollable/canvas area by
/// this much when a paginated layout contains comment items, so the panel is
/// reachable. See [`result::LayoutPage::comment_items`].
pub const COMMENT_GUTTER_WIDTH: f32 = 192.0;

/// Options that control the layout pipeline's memory / feature trade-offs.
///
/// Pass to [`layout_document`] or [`flow_section`]. The default (all fields
/// `false`) is the read-only rendering mode — zero overhead for features the
/// renderer does not need.
#[derive(Debug, Clone, Default)]
pub struct LayoutOptions {
    /// When `true`, the Parley `Layout` object is retained inside each
    /// [`ParagraphLayout`] so that [`ParagraphLayout::hit_test_point`] and
    /// [`ParagraphLayout::cursor_rect`] can be called afterwards.
    ///
    /// Has a memory cost proportional to document size. Use `false` (the
    /// default) for read-only document viewing. Editing sessions pass `true`.
    pub preserve_for_editing: bool,

    /// Optional spell checker. When `Some`, each paragraph's text is checked and
    /// misspelled words emit a [`items::DecorationKind::Spelling`] squiggle
    /// decoration (positioned via the same Parley selection-geometry mechanism
    /// as the highlight underlay). `None` (the default) adds zero overhead.
    pub spell: Option<SpellState>,
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

/// Lays out a full document into absolute positions.
///
/// This processes all sections in the document and returns a single [`DocumentLayout`].
/// In the current implementation, sections are stacked vertically.
///
/// Pass [`LayoutOptions::default()`] for read-only rendering. Pass
/// `LayoutOptions { preserve_for_editing: true, .. }` when the result
/// needs to support [`ParagraphLayout::hit_test_point`] /
/// [`ParagraphLayout::cursor_rect`].
pub fn layout_document(
    resources: &mut FontResources,
    doc: &loki_doc_model::Document,
    mode: LayoutMode,
    display_scale: f32,
    options: &LayoutOptions,
) -> DocumentLayout {
    match mode {
        LayoutMode::Paginated => DocumentLayout::Paginated(
            layout_paginated_full(resources, doc, display_scale, options).0,
        ),
        _ => {
            let mut all_items = Vec::new();
            let mut all_paragraphs = Vec::new();
            let mut total_height = 0.0;
            let mut max_width: f32 = 0.0;
            // Running base so block indices are *global* (document order across
            // every section), matching the index space the editor and the
            // `loro_mutation` layer use to address blocks.
            let mut block_base = 0usize;

            for section in &doc.sections {
                let FlowOutput::Canvas {
                    mut items,
                    height,
                    mut paragraphs,
                    ..
                } = flow_section(
                    resources,
                    section,
                    &doc.styles,
                    &mode,
                    display_scale,
                    options,
                    &doc.comments,
                )
                else {
                    unreachable!("flow_section in non-paginated mode always returns Canvas");
                };
                // Offset section items (and editing origins) by the height of
                // all preceding sections so coordinates are canvas-absolute.
                for item in &mut items {
                    item.translate(0.0, total_height);
                }
                for para in &mut paragraphs {
                    para.origin.1 += total_height;
                    para.block_index += block_base;
                }
                all_items.extend(items);
                all_paragraphs.extend(paragraphs);
                total_height += height;
                block_base += section.blocks.len();

                let pl = &section.layout;
                let page_w = pts_to_f32(pl.page_size.width);
                max_width = max_width.max(page_w);
            }

            DocumentLayout::Continuous(ContinuousLayout {
                content_width: match mode {
                    LayoutMode::Reflow { available_width } => available_width,
                    _ => max_width,
                },
                total_height,
                items: all_items,
                paragraphs: all_paragraphs,
            })
        }
    }
}

/// Full paginated layout that also returns the clean-page-top checkpoints used
/// by [`relayout_paginated_incremental`].
///
/// Identical output to `layout_document(.., Paginated, ..)`; the second tuple
/// element is the reuse metadata. Checkpoints are only returned for
/// single-section documents (the incremental path's eligibility); multi-section
/// documents return an empty checkpoint list, which simply disables incremental
/// reuse for them.
pub fn layout_paginated_full(
    resources: &mut FontResources,
    doc: &loki_doc_model::Document,
    display_scale: f32,
    options: &LayoutOptions,
) -> (PaginatedLayout, PaginatedReuse) {
    let mode = LayoutMode::Paginated;
    let mut global_page_count = 0;
    // Running base so editing block indices are global across sections (see the
    // continuous path and the `loro_mutation` resolver).
    let mut block_base = 0usize;
    let mut first_page_size = None;
    let mut checkpoints: Vec<PageStart> = Vec::new();

    // Partition sections into page-sharing GROUPS: a new group begins at the
    // first section and at every section that does *not* start `continuous`. A
    // `continuous` section continues on the previous group's last page (sharing
    // its page geometry + headers/footers), only switching column layout.
    let mut groups: Vec<Vec<&loki_doc_model::Section>> = Vec::new();
    for section in &doc.sections {
        match groups.last_mut() {
            Some(last) if section.start == loki_doc_model::layout::SectionStart::Continuous => {
                last.push(section);
            }
            _ => groups.push(vec![section]),
        }
    }

    // Pass 1: flow every group's body so the total page count is known before
    // headers/footers are laid out (NUMPAGES fields need the document-wide total).
    // Each group is laid out as one page sequence owned by its first section.
    let mut flowed: Vec<(&loki_doc_model::Section, Vec<LayoutPage>)> = Vec::new();
    // Document-section index of the current group's first section (for checkpoint
    // tagging / incremental).
    let mut primary_section_index = 0usize;
    for group in &groups {
        let primary = group[0];
        let pl = &primary.layout;
        let page_size = LayoutSize::new(
            pts_to_f32(pl.page_size.width),
            pts_to_f32(pl.page_size.height),
        );
        if first_page_size.is_none() {
            first_page_size = Some(page_size);
        }

        let FlowOutput::Pages {
            mut pages,
            checkpoints: group_checkpoints,
            ..
        } = flow::flow_section_group(
            resources,
            group,
            &doc.styles,
            &mode,
            display_scale,
            options,
            &doc.comments,
        )
        else {
            unreachable!("flow_section_group in Paginated mode always returns Pages");
        };

        let group_blocks: usize = group.iter().map(|s| s.blocks.len()).sum();
        let group_page_count = pages.len();
        // Renumber pages so select_header/select_footer receive the correct
        // absolute page number for first/even selection.
        for page in &mut pages {
            page.page_number += global_page_count;
            // Globalise editing block indices across groups so hit-test / cursor
            // positions resolve to the right block (group-local → document).
            if let Some(ed) = page.editing_data.as_mut() {
                for para in &mut ed.paragraphs {
                    para.block_index += block_base;
                }
            }
        }
        // Lift the group-local checkpoints to document-global.
        for mut cp in group_checkpoints {
            cp.section_index = primary_section_index;
            cp.page_index += global_page_count;
            checkpoints.push(cp);
        }

        flowed.push((primary, pages));
        global_page_count += group_page_count;
        block_base += group_blocks;
        primary_section_index += group.len();
    }

    // Pass 2: headers/footers, with the document-wide page total available for
    // PAGE / NUMPAGES field substitution.
    let mut all_pages = Vec::new();
    for (section, mut pages) in flowed {
        flow::assign_headers_footers(
            &mut pages,
            &section.layout,
            resources,
            &doc.styles,
            display_scale,
            global_page_count as u32,
        );
        all_pages.extend(pages);
    }

    (
        PaginatedLayout {
            page_size: first_page_size.unwrap_or_default(),
            pages: all_pages.into_iter().map(std::sync::Arc::new).collect(),
        },
        PaginatedReuse {
            checkpoints,
            has_footnotes: incremental::document_has_notes(doc),
        },
    )
}
