// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 AppThere Loki contributors

//! Top-level layout entry points (split from `lib.rs` for the 300-line
//! ceiling): `layout_document` (paginated or continuous) and
//! `layout_paginated_full` (paginated + the incremental-reuse checkpoints),
//! plus `effective_options` which folds document settings into the caller's
//! `LayoutOptions`. Re-exported from the crate root.

use crate::flow::{self, FlowOutput, flow_section};
use crate::font::FontResources;
use crate::geometry::LayoutSize;
use crate::incremental::{self, PageStart, PaginatedReuse};
use crate::mode::LayoutMode;
use crate::options::LayoutOptions;
use crate::paginate_blanks;
use crate::resolve::pts_to_f32;
use crate::result::{ContinuousLayout, DocumentLayout, LayoutPage, PaginatedLayout};

/// Fold document-level [`loki_doc_model::settings::DocumentSettings`] into the
/// caller's [`LayoutOptions`], filling any field the caller left unset.
///
/// Derives `default_tab_stop_pt` and `mirror_margins`; a caller-supplied
/// value takes precedence, and a document with no `settings` leaves the
/// built-in fallbacks in place.
fn effective_options(doc: &loki_doc_model::Document, options: &LayoutOptions) -> LayoutOptions {
    let mut eff = options.clone();
    if eff.default_tab_stop_pt.is_none() {
        eff.default_tab_stop_pt = doc.settings.as_ref().map(|s| s.default_tab_stop_pt);
    }
    eff.mirror_margins |= doc.settings.as_ref().is_some_and(|s| s.mirror_margins);
    eff
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
    let effective = effective_options(doc, options);
    let options = &effective;
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
    let effective = effective_options(doc, options);
    let options = &effective;
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
    //
    // Exception (Word fidelity): a `continuous` break that *changes the page
    // size or orientation* cannot share the previous page — Word promotes it to
    // a page break so the new geometry can take effect. Without this, e.g. an
    // A4 continuous section after a Letter section is laid out on the Letter
    // page (wrong geometry, one page short).
    let mut groups: Vec<Vec<&loki_doc_model::Section>> = Vec::new();
    for section in &doc.sections {
        match groups.last_mut() {
            Some(last)
                if section.start == loki_doc_model::layout::SectionStart::Continuous
                    && section.layout.page_size == last[0].layout.page_size =>
            {
                last.push(section);
            }
            _ => groups.push(vec![section]),
        }
    }

    // Pass 1: flow every group's body so the total page count is known before
    // headers/footers are laid out (NUMPAGES fields need the document-wide total).
    // Each group is laid out as one page sequence owned by its first section.
    let mut flowed: Vec<(
        &loki_doc_model::Section,
        Vec<LayoutPage>,
        Option<LayoutPage>,
    )> = Vec::new();
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

        // Even/odd section break: insert a blank filler page (counted now, so the
        // section's own pages are numbered after it) when the section would
        // otherwise start on the wrong parity.
        let leading_blank = if paginate_blanks::needs_blank_before(primary.start, global_page_count)
        {
            let bp = paginate_blanks::blank_page(global_page_count + 1, &primary.layout);
            global_page_count += 1;
            Some(bp)
        } else {
            None
        };

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

        flowed.push((primary, pages, leading_blank));
        global_page_count += group_page_count;
        block_base += group_blocks;
        primary_section_index += group.len();
    }

    // Pass 2: headers/footers, with the document-wide page total available for
    // PAGE / NUMPAGES field substitution.
    let mut all_pages = Vec::new();
    for (section, mut pages, blank) in flowed {
        // Even/odd filler page (no header/footer) precedes the section's pages.
        if let Some(bp) = blank {
            all_pages.push(bp);
        }
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
