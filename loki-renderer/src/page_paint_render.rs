// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 AppThere Loki contributors

//! Self-contained phases of `LokiPageSource::render`, split out of
//! `page_paint_source.rs` for the 300-line ceiling. Both are pure helpers that
//! take only the data the render loop passes in — no `LokiPageSource` state is
//! reached except through the `source` handle.

use anyrender_vello::wgpu::{
    Device, Extent3d, Texture, TextureDimension, TextureFormat, TextureUsages, TextureView,
    TextureViewDescriptor,
};

use crate::doc_page_source::DocPageSource;
use crate::document_view::RendererSelection;

/// Allocate the per-page GPU texture and its default view.
///
// COMPAT(blitz): Rgba8Unorm + STORAGE_BINDING|TEXTURE_BINDING matches the
// format expected by anyrender_vello `register_texture`; COPY_SRC allows the
// composited read-back path.
pub(super) fn allocate_page_texture(
    device: &Device,
    w_phys: u32,
    h_phys: u32,
) -> (Texture, TextureView) {
    let texture = device.create_texture(&anyrender_vello::wgpu::TextureDescriptor {
        label: Some("loki-page"),
        size: Extent3d {
            width: w_phys,
            height: h_phys,
            depth_or_array_layers: 1,
        },
        mip_level_count: 1,
        sample_count: 1,
        dimension: TextureDimension::D2,
        format: TextureFormat::Rgba8Unorm,
        usage: TextureUsages::STORAGE_BINDING
            | TextureUsages::TEXTURE_BINDING
            | TextureUsages::COPY_SRC,
        view_formats: &[],
    });
    let view = texture.create_view(&TextureViewDescriptor::default());
    (texture, view)
}

/// Compute page-relative cursor paint data for this page.
///
/// Returns `None` when there is no selection, the caret is on another page, or
/// the layout for `generation` carries no editing data (reflow layouts). The
/// layout guard is scoped inside so it is dropped before the caller re-locks
/// the layout for the paint pass.
pub(super) fn page_cursor_paint(
    source: &DocPageSource,
    page_index: usize,
    generation: u64,
    current_sel: Option<RendererSelection>,
) -> Option<loki_vello::CursorPaint> {
    current_sel.and_then(|sel| {
        let cp = sel.focus;
        if cp.page_index != page_index {
            return None;
        }
        let guard = source.layout_for_generation(generation);
        // Reflow layouts carry no editing data — no cursor is painted.
        let layout = guard.as_ref()?.1.as_paginated()?;
        let page = layout.pages.get(page_index)?;
        let editing_data = page.editing_data.as_ref()?;
        let para_data = editing_data
            .paragraphs
            .iter()
            .find(|p| p.block_index == cp.paragraph_index)?;
        let cursor_rect = para_data.layout.cursor_rect(cp.byte_offset);
        Some(loki_vello::CursorPaint {
            cursor_rect,
            selection_rects: vec![],
            selection_handles: vec![],
            paragraph_index: cp.paragraph_index,
        })
    })
}
