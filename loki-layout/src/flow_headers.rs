// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 AppThere Loki contributors

//! Header/footer layout orchestration (split from `flow.rs` for the 300-line
//! ceiling): `layout_blocks_reflow` lays a block stream out once in reflow
//! mode, and `assign_headers_footers` selects the first/even/default variant
//! per page, re-laying-out page-field variants ("Page X of Y") per page and
//! translating each into page-local coordinates. Both are re-exported from
//! `flow.rs` so their existing paths are unchanged.

use loki_doc_model::content::attr::ExtensionBag;
use loki_doc_model::content::block::Block;
use loki_doc_model::layout::header_footer::HeaderFooter;
use loki_doc_model::layout::page::{PageBorders, PageLayout};
use loki_doc_model::{Section, StyleCatalog};

use super::{FlowOutput, flow_section, page_fields};
use crate::font::FontResources;
use crate::geometry::LayoutRect;
use crate::items::{PositionedBorderRect, PositionedItem};
use crate::mode::LayoutMode;
use crate::resolve::{convert_border, pts_to_f32};
use crate::result::LayoutPage;

/// Lay out `blocks` in reflow mode using `available_width`.
///
/// Returns the positioned items (in `(0,0)`-origin canvas coordinates) and the
/// total canvas height. Items have no Y offset applied — the caller translates
/// them to page-local coordinates.
pub(crate) fn layout_blocks_reflow(
    resources: &mut FontResources,
    blocks: &[Block],
    catalog: &StyleCatalog,
    available_width: f32,
    display_scale: f32,
    field_context: Option<crate::FieldContext>,
) -> (Vec<PositionedItem>, f32) {
    use crate::LayoutOptions;
    let mut blocks = blocks.to_vec();
    // Substitute PAGE / NUMPAGES fields with their resolved values before
    // layout — the blocks are already a per-call clone, so this never
    // mutates the document.
    if let Some(ctx) = field_context {
        page_fields::substitute_page_fields(&mut blocks, &ctx);
    }
    let synthetic = Section {
        layout: PageLayout::default(),
        blocks,
        start: loki_doc_model::layout::section::SectionStart::default(),
        page_style: None,
        extensions: ExtensionBag::default(),
    };
    let mode = LayoutMode::Reflow { available_width };
    let options = LayoutOptions::default(); // headers/footers read-only here
    match flow_section(
        resources,
        &synthetic,
        catalog,
        &mode,
        display_scale,
        &options,
        &[],
    ) {
        FlowOutput::Canvas { items, height, .. } => (items, height),
        FlowOutput::Pages { .. } => unreachable!("Reflow mode always returns Canvas"),
    }
}

/// A variant laid out once and shared by every page that shows it, or `None`
/// when it contains a PAGE / NUMPAGES field and must be re-laid-out per page.
type Prelaid = Option<(Vec<PositionedItem>, f32)>;

/// The first / even / default variants of one region, each with its pre-laid
/// items — what [`Variants::select`] chooses between.
struct Variants<'a> {
    first: (&'a Option<HeaderFooter>, &'a Option<Prelaid>),
    even: (&'a Option<HeaderFooter>, &'a Option<Prelaid>),
    default: (&'a Option<HeaderFooter>, &'a Option<Prelaid>),
}

impl<'a> Variants<'a> {
    /// The variant page `pn` shows.
    ///
    /// `is_first` is the caller's answer to "is this the first page of *this
    /// section*" — not `pn == 1`, which asks about the document and is what
    /// stopped every section after the first from ever showing its first-page
    /// variant.
    fn select(&self, pn: usize, is_first: bool) -> Option<(&'a HeaderFooter, &'a Prelaid)> {
        let pick = if is_first && self.first.0.is_some() {
            self.first
        } else if pn.is_multiple_of(2) && self.even.0.is_some() {
            self.even
        } else {
            self.default
        };
        pick.0.as_ref().zip(pick.1.as_ref())
    }
}

/// Where the section being laid out sits in the finished document.
///
/// The two travel together because neither can be derived from the `pages`
/// slice: it is not necessarily the whole document (so the total is unknown)
/// and not necessarily the whole section (so its start is unknown).
#[derive(Clone, Copy)]
pub(crate) struct PagePosition {
    /// Document-global page number of this section's **first** page, 1-indexed.
    ///
    /// Two quantities depend on it: which page gets the first-page variant, and
    /// where a `w:pgNumType` restart counts from.
    ///
    /// Supplied rather than read off `pages.first()` because `pages` is not
    /// guaranteed to be the whole section — the incremental path passes only the
    /// re-flowed middle. *Today* that middle always is the whole section (there
    /// is one checkpoint per section, at block 0, so every resume starts there —
    /// see `the_property_tests_only_ever_resume_from_block_zero`), so inferring
    /// it from the slice would give the same answer. Once T3.4 makes checkpoints
    /// per-page, resumes begin mid-section and the inference silently becomes
    /// wrong for both quantities. Taking it from the caller, who knows it either
    /// way, is what stops that from being a change nobody remembers to make.
    pub section_first_page: usize,
    /// Total pages in the finished document, for NUMPAGES.
    pub total_page_count: u32,
}

/// Populate header/footer items for each page in `pages`.
///
/// Variants without PAGE / NUMPAGES fields are laid out once (in reflow mode)
/// and cloned onto each page. Variants containing page fields are re-laid-out
/// per page with a [`crate::FieldContext`] carrying the real page number and
/// `pos.total_page_count`, so "Page X of Y" chrome renders correctly.
///
/// Items are translated to page-local coords: header top `margins.header`;
/// footer top `page_height - margins.footer - footer_height`.
pub(crate) fn assign_headers_footers(
    pages: &mut [LayoutPage],
    layout: &PageLayout,
    resources: &mut FontResources,
    catalog: &StyleCatalog,
    display_scale: f32,
    pos: PagePosition,
) {
    let content_width = pages
        .first()
        .map(|p| (p.page_size.width - p.margins.horizontal()).max(0.0))
        .unwrap_or(0.0);

    // Lay out a variant once when it has no page fields; `None` marks
    // variants that must be re-laid-out per page.
    let mut lay_static = |hf: &HeaderFooter| -> Option<(Vec<PositionedItem>, f32)> {
        if page_fields::blocks_contain_page_field(&hf.blocks) {
            None
        } else {
            Some(layout_blocks_reflow(
                resources,
                &hf.blocks,
                catalog,
                content_width,
                display_scale,
                None,
            ))
        }
    };

    let hdr_default = layout.header.as_ref().map(&mut lay_static);
    let hdr_first = layout.header_first.as_ref().map(&mut lay_static);
    let hdr_even = layout.header_even.as_ref().map(&mut lay_static);
    let ftr_default = layout.footer.as_ref().map(&mut lay_static);
    let ftr_first = layout.footer_first.as_ref().map(&mut lay_static);
    let ftr_even = layout.footer_even.as_ref().map(&mut lay_static);

    let hdr_margin_y = pts_to_f32(layout.margins.header);
    let ftr_margin = pts_to_f32(layout.margins.footer);
    let left_margin = pts_to_f32(layout.margins.left);

    // The three variants of one region (header or footer), each paired with its
    // pre-laid items. Grouped into a struct rather than passed as six loose
    // arguments: they arrive in first/even/default order twice over, and a
    // transposed pair between the two calls would type-check.
    let headers = Variants {
        first: (&layout.header_first, &hdr_first),
        even: (&layout.header_even, &hdr_even),
        default: (&layout.header, &hdr_default),
    };
    let footers = Variants {
        first: (&layout.footer_first, &ftr_first),
        even: (&layout.footer_even, &ftr_even),
        default: (&layout.footer, &ftr_default),
    };

    for page in pages.iter_mut() {
        let page_h = page.page_size.height;
        let pn = page.page_number;
        // Apply the section restart: the section's first page shows `start`, and
        // following pages increment from there. Absent a restart, use the
        // document-global physical page number.
        let display_pn = match layout.page_number_start {
            Some(start) => start as usize + pn.saturating_sub(pos.section_first_page),
            None => pn,
        };
        let ctx = crate::FieldContext {
            page_number: display_pn as u32,
            page_count: pos.total_page_count,
            number_format: layout.page_number_format,
        };

        let is_first = pn == pos.section_first_page;
        let hdr = headers.select(pn, is_first);
        let ftr = footers.select(pn, is_first);

        if let Some((hf, pre)) = hdr {
            let (mut items, h) = match pre {
                Some((items, h)) => (items.clone(), *h),
                // Contains page fields — lay out fresh for this page.
                None => layout_blocks_reflow(
                    resources,
                    &hf.blocks,
                    catalog,
                    content_width,
                    display_scale,
                    Some(ctx),
                ),
            };
            for item in &mut items {
                item.translate(left_margin, hdr_margin_y);
            }
            page.header_items = items;
            page.header_height = h;
        }

        if let Some((hf, pre)) = ftr {
            let (mut items, h) = match pre {
                Some((items, h)) => (items.clone(), *h),
                None => layout_blocks_reflow(
                    resources,
                    &hf.blocks,
                    catalog,
                    content_width,
                    display_scale,
                    Some(ctx),
                ),
            };
            let footer_y = page_h - ftr_margin - h;
            for item in &mut items {
                item.translate(left_margin, footer_y);
            }
            page.footer_items = items;
            page.footer_height = h;
        }

        // Page border (`w:pgBorders`): a decoration drawn around every page of
        // the section, in the margin area. It goes in the page-local, unclipped
        // item list (like headers/footers) rather than the content area.
        if let Some(pb) = layout.page_border.as_ref() {
            page.header_items.push(page_border_item(page, pb, layout));
        }
    }
}

/// Builds the page-border decoration rect for one page, in page-local coords.
///
/// Each edge is inset from the physical page edge by its `w:space` (points);
/// with `offset_from_text` the inset is measured inward from the text/margin
/// area instead. Absent-spacing edges fall back to Word's 24 pt default.
fn page_border_item(page: &LayoutPage, pb: &PageBorders, layout: &PageLayout) -> PositionedItem {
    const DEFAULT_SPACE: f32 = 24.0;
    let (pw, ph) = (page.page_size.width, page.page_size.height);
    let space = |e: &Option<loki_doc_model::style::props::border::Border>| {
        e.as_ref()
            .and_then(|b| b.spacing)
            .map_or(DEFAULT_SPACE, pts_to_f32)
    };
    // Inset of one side from the page edge (points).
    let inset = |e: &Option<loki_doc_model::style::props::border::Border>, margin: f32| {
        if pb.offset_from_text {
            (margin - space(e)).max(0.0)
        } else {
            space(e)
        }
    };
    let l = inset(&pb.left, pts_to_f32(layout.margins.left));
    let t = inset(&pb.top, pts_to_f32(layout.margins.top));
    let r = pw - inset(&pb.right, pts_to_f32(layout.margins.right));
    let b = ph - inset(&pb.bottom, pts_to_f32(layout.margins.bottom));

    PositionedItem::BorderRect(PositionedBorderRect {
        rect: LayoutRect::new(l, t, (r - l).max(0.0), (b - t).max(0.0)),
        top: pb.top.as_ref().and_then(convert_border),
        right: pb.right.as_ref().and_then(convert_border),
        bottom: pb.bottom.as_ref().and_then(convert_border),
        left: pb.left.as_ref().and_then(convert_border),
    })
}
