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

/// Compute page-relative cursor paint data for this page: the caret (when the
/// focus is on this page) plus per-paragraph selection highlight spans for
/// whatever part of the (possibly multi-paragraph, multi-page) selection is
/// visible here.
///
/// Returns `None` when there is nothing to paint on this page, or the layout
/// for `generation` carries no editing data (reflow layouts). The layout guard
/// is scoped inside so it is dropped before the caller re-locks the layout for
/// the paint pass.
pub(super) fn page_cursor_paint(
    source: &DocPageSource,
    page_index: usize,
    generation: u64,
    current_sel: Option<RendererSelection>,
) -> Option<loki_vello::CursorPaint> {
    let sel = current_sel?;
    let guard = source.layout_for_generation(generation);
    // Reflow layouts carry no editing data — no cursor is painted.
    let layout = guard.as_ref()?.1.as_paginated()?;
    let page = layout.pages.get(page_index)?;
    let editing_data = page.editing_data.as_ref()?;

    // Caret: only on the focus's page.
    let cursor_rect = (sel.focus.page_index == page_index)
        .then(|| {
            editing_data
                .paragraphs
                .iter()
                .find(|p| p.block_index == sel.focus.paragraph_index)
                .and_then(|p| p.layout.cursor_rect(sel.focus.byte_offset))
        })
        .flatten();

    let selection_spans = if sel.is_collapsed() {
        vec![]
    } else {
        selection_spans_for_page(page, &sel)
    };

    if cursor_rect.is_none() && selection_spans.is_empty() {
        return None;
    }
    Some(loki_vello::CursorPaint {
        cursor_rect,
        selection_rects: vec![],
        selection_handles: vec![],
        selection_spans,
        paragraph_index: sel.focus.paragraph_index,
    })
}

/// Builds the per-paragraph highlight spans for the part of `sel` that is
/// visible on `page`. Selection endpoints are ordered document-forward; a
/// paragraph strictly inside the range is fully selected. Rects are clipped to
/// the page's content band (a paragraph split across pages registers the same
/// layout on both pages, at shifted origins). Table-cell paragraphs (non-empty
/// path) are skipped — the selection endpoints carry no path.
fn selection_spans_for_page(
    page: &loki_layout::LayoutPage,
    sel: &RendererSelection,
) -> Vec<loki_vello::SelectionSpan> {
    let Some(editing_data) = page.editing_data.as_ref() else {
        return vec![];
    };
    let (start, end) = {
        let a = (sel.anchor.paragraph_index, sel.anchor.byte_offset);
        let f = (sel.focus.paragraph_index, sel.focus.byte_offset);
        if a <= f { (a, f) } else { (f, a) }
    };
    let content_h = page.page_size.height - page.margins.top - page.margins.bottom;

    let mut spans = Vec::new();
    for para in &editing_data.paragraphs {
        if para.block_index < start.0 || para.block_index > end.0 || !para.path.is_empty() {
            continue;
        }
        let from = if para.block_index == start.0 {
            start.1
        } else {
            0
        };
        let to = if para.block_index == end.0 {
            end.1
        } else {
            usize::MAX // clamped by selection_rects
        };
        let rects: Vec<loki_vello::SelectionRect> = para
            .layout
            .selection_rects(from, to)
            .into_iter()
            // Clip to this page's content band (page-split fragments).
            .filter(|r| {
                let top = r.origin.y + para.origin.1;
                top + r.size.height > 0.0 && top < content_h
            })
            .map(|r| loki_vello::SelectionRect {
                x: r.origin.x,
                y: r.origin.y,
                width: r.size.width,
                height: r.size.height,
            })
            .collect();
        if rects.is_empty() {
            continue;
        }
        let handles = selection_handles_for_span(para.block_index, &rects, start.0, end.0);
        spans.push(loki_vello::SelectionSpan {
            paragraph_index: para.block_index,
            rects,
            handles,
        });
    }
    spans
}

/// Teardrop drag handles at the selection edges — mobile only (idiomatic for
/// touch; on desktop the highlight alone is standard).
#[cfg(target_os = "android")]
fn selection_handles_for_span(
    block_index: usize,
    rects: &[loki_vello::SelectionRect],
    start_block: usize,
    end_block: usize,
) -> Vec<loki_vello::SelectionHandle> {
    let mut handles = Vec::new();
    if block_index == start_block
        && let Some(first) = rects.first()
    {
        handles.push(loki_vello::SelectionHandle {
            tip_x: first.x,
            tip_y: first.y + first.height,
            kind: loki_vello::SelectionHandleKind::Anchor,
        });
    }
    if block_index == end_block
        && let Some(last) = rects.last()
    {
        handles.push(loki_vello::SelectionHandle {
            tip_x: last.x + last.width,
            tip_y: last.y + last.height,
            kind: loki_vello::SelectionHandleKind::Focus,
        });
    }
    handles
}

#[cfg(not(target_os = "android"))]
fn selection_handles_for_span(
    _block_index: usize,
    _rects: &[loki_vello::SelectionRect],
    _start_block: usize,
    _end_block: usize,
) -> Vec<loki_vello::SelectionHandle> {
    Vec::new()
}
