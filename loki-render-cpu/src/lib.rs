// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 AppThere Loki contributors

//! Deterministic in-process CPU rasterization of [`loki_layout`] output —
//! the Spec 02 **D2** conformance *candidate* render path.
//!
//! Loki's production render path is Vello on wgpu (GPU): fast, but not
//! bit-reproducible across drivers, and unusable in the GPU-less CI/agent
//! environment. This crate renders the **same positioned items the GPU path
//! paints** (`PositionedItem` from `loki-layout`) through `vello_cpu`, a pure
//! software rasterizer needing no graphics adapter — so committed goldens
//! compare against a deterministic candidate.
//!
//! Divergence containment (Spec 02 §7.1): both paths consume the identical
//! renderer-agnostic layout output; each `PositionedItem` arm here mirrors
//! its `loki-vello` twin (same origin/offset/scale math, same skip rules).
//! The differences are confined to the rasterizer itself — which is fine,
//! because fidelity is judged against the *reference application's* golden,
//! never against Loki's GPU output.
//!
//! Deliberately not rendered (editor chrome, not document content): page
//! drop shadows, cursors, selection highlights, spell squiggles' hover
//! state. TODO(conformance-render): image items paint the same grey
//! placeholder as `loki-vello`'s unresolved-image path; decoding embedded
//! images is a follow-up.

#![forbid(unsafe_code)]

mod paint;

use image::RgbaImage;
use loki_layout::{LayoutRect, PaginatedLayout, PositionedItem, PositionedRect};
use vello_cpu::{Pixmap, RenderContext, Resources};

/// Errors from the CPU candidate render.
#[derive(Debug, thiserror::Error)]
pub enum RenderCpuError {
    /// The requested page does not exist in the layout.
    #[error("page {index} out of range ({pages} pages)")]
    PageOutOfRange {
        /// Requested page index.
        index: usize,
        /// Number of pages in the layout.
        pages: usize,
    },
    /// The page dimensions overflow the rasterizer's u16 pixel space.
    #[error("page pixel size {0}x{1} exceeds the rasterizer limit (65535)")]
    PageTooLarge(u32, u32),
}

/// White paper, matching the production painter's page background.
const PAGE_BG: loki_layout::LayoutColor = loki_layout::LayoutColor::WHITE;

/// Renders one page of a paginated layout to an RGBA image at `dpi`.
///
/// The geometry mirrors `loki_vello::paint_single_page`: content items are
/// content-area-local (translated by the page margins), header/footer and
/// comment items are page-local. No editor chrome (shadow/cursor) is drawn.
pub fn render_page(
    layout: &PaginatedLayout,
    page_index: usize,
    dpi: u32,
) -> Result<RgbaImage, RenderCpuError> {
    let page = layout
        .pages
        .get(page_index)
        .ok_or(RenderCpuError::PageOutOfRange {
            index: page_index,
            pages: layout.pages.len(),
        })?;

    let scale = dpi as f32 / 72.0;
    let (page_w, page_h) = (page.page_size.width, page.page_size.height);
    let px_w = (page_w * scale).ceil() as u32;
    let px_h = (page_h * scale).ceil() as u32;
    if px_w > u32::from(u16::MAX) || px_h > u32::from(u16::MAX) || px_w == 0 || px_h == 0 {
        return Err(RenderCpuError::PageTooLarge(px_w, px_h));
    }

    let mut ctx = RenderContext::new(px_w as u16, px_h as u16);
    let mut resources = Resources::new();

    // Paper background across the full pixmap.
    paint::paint_filled_rect(
        &mut ctx,
        &PositionedRect {
            rect: LayoutRect::new(0.0, 0.0, page_w, page_h),
            color: PAGE_BG,
        },
        scale,
        (0.0, 0.0),
    );

    // Mirrors paint_single_page's coordinate spaces.
    let page_origin = (0.0, 0.0);
    let content_origin = (page.margins.left, page.margins.top);
    paint::paint_items(
        &mut ctx,
        &mut resources,
        &page.content_items,
        scale,
        content_origin,
    );
    paint::paint_items(
        &mut ctx,
        &mut resources,
        &page.header_items,
        scale,
        page_origin,
    );
    paint::paint_items(
        &mut ctx,
        &mut resources,
        &page.footer_items,
        scale,
        page_origin,
    );
    paint::paint_items(
        &mut ctx,
        &mut resources,
        &page.comment_items,
        scale,
        page_origin,
    );

    ctx.flush();
    let mut pixmap = Pixmap::new(px_w as u16, px_h as u16);
    ctx.render_to_pixmap(&mut resources, &mut pixmap);

    let data = pixmap.data_as_u8_slice().to_vec();
    Ok(RgbaImage::from_raw(px_w, px_h, data)
        .expect("pixmap dimensions match the buffer by construction"))
}

/// Renders every page of the layout, in page order.
pub fn render_document(
    layout: &PaginatedLayout,
    dpi: u32,
) -> Result<Vec<RgbaImage>, RenderCpuError> {
    (0..layout.pages.len())
        .map(|i| render_page(layout, i, dpi))
        .collect()
}

/// Reachable item count across groups — used by tests/diagnostics to assert a
/// layout actually produced paintable content.
#[must_use]
pub fn paintable_item_count(items: &[PositionedItem]) -> usize {
    items
        .iter()
        .map(|i| match i {
            PositionedItem::ClippedGroup { items, .. } => paintable_item_count(items),
            PositionedItem::RotatedGroup { items, .. } => paintable_item_count(items),
            _ => 1,
        })
        .sum()
}
