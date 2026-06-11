// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 AppThere Loki contributors

//! [`PageTile`] — one render tile (a paginated page or a reflow band) painted
//! into a Blitz GPU canvas.  Split from `document_view.rs` to stay under the
//! file-size ceiling.

use std::sync::{Arc, Mutex};

use appthere_canvas::{PageCache, PageIndex};
use dioxus::native::use_wgpu;
use dioxus::prelude::*;

use crate::doc_page_source::DocPageSource;
use crate::document_view::RendererCursorPos;
use crate::page_paint_source::LokiPageSource;

#[derive(Clone, Props)]
pub(crate) struct PageTileProps {
    pub(crate) cache: Arc<Mutex<PageCache<PageIndex>>>,
    pub(crate) source: Arc<DocPageSource>,
    pub(crate) page_index: usize,
    pub(crate) w: f64,
    pub(crate) h: f64,
    pub(crate) shared_renderer: Arc<Mutex<Option<vello::Renderer>>>,
    pub(crate) cursor_holder: Arc<Mutex<Option<RendererCursorPos>>>,
    pub(crate) cursor_pos: Option<RendererCursorPos>,
    /// Document generation — incremented on every mutation so that style
    /// changes that don't move the cursor still dirty the canvas.
    pub(crate) doc_gen: u64,
    /// Settle epoch — incremented after each scroll-settle retier so that a
    /// tier change repaints this tile at its new resolution.
    pub(crate) settle_epoch: u64,
    /// Vertical gap below this tile in CSS px — the page gap in paginated
    /// mode, `0` in reflow mode so bands stitch into a continuous flow.
    pub(crate) gap_px: f64,
    /// Called with `(page_index, x_pt, y_pt)` in layout points when the user
    /// clicks anywhere on this page tile. The parent uses this to call
    /// `hit_test_page` without needing window-relative origin math.
    pub(crate) on_tile_click: EventHandler<(usize, f32, f32)>,
}

impl PartialEq for PageTileProps {
    fn eq(&self, other: &Self) -> bool {
        Arc::ptr_eq(&self.cache, &other.cache)
            && Arc::ptr_eq(&self.source, &other.source)
            && self.page_index == other.page_index
            && self.w == other.w
            && self.h == other.h
            && Arc::ptr_eq(&self.cursor_holder, &other.cursor_holder)
            && self.cursor_pos == other.cursor_pos
            && self.doc_gen == other.doc_gen
            && self.settle_epoch == other.settle_epoch
            && self.gap_px == other.gap_px
        // on_tile_click intentionally excluded — EventHandler identity does not
        // affect render output; omitting it avoids spurious re-renders.
    }
}

/// A single page rendered into a Blitz GPU canvas.
#[allow(non_snake_case)]
pub(crate) fn PageTile(props: PageTileProps) -> Element {
    let cache = props.cache.clone();
    let source = props.source.clone();
    let page_index = props.page_index;
    let shared_renderer = props.shared_renderer.clone();
    let cursor_holder = props.cursor_holder.clone();
    let cursor_holder_wgpu = props.cursor_holder.clone();
    let on_tile_click = props.on_tile_click;

    // Write current cursor to shared holder on every render so LokiPageSource
    // can read it during the GPU paint call.
    if let Ok(mut guard) = cursor_holder.lock() {
        *guard = props.cursor_pos;
    }

    let canvas_id = use_wgpu(move || {
        LokiPageSource::new(
            cache,
            source,
            page_index,
            shared_renderer,
            cursor_holder_wgpu,
        )
    });

    // Combines cursor position and document generation so that both cursor
    // movement AND document mutations (e.g. style changes that don't shift
    // the cursor) mark the canvas dirty and trigger render() again.
    // COMPAT(dioxus-native): data-* attributes confirmed working in Blitz.
    let data_cursor = match props.cursor_pos {
        Some(cp) if cp.page_index == props.page_index => {
            format!(
                "{}-{}-{}-{}",
                cp.paragraph_index, cp.byte_offset, props.doc_gen, props.settle_epoch
            )
        }
        _ => format!("{}-{}", props.doc_gen, props.settle_epoch),
    };

    rsx! {
        div {
            // COMPAT(dioxus-native): position:absolute is unsupported in Blitz.
            // Use block flow with auto margins for horizontal centring instead.
            style: format!(
                "display: block; width: {w}px; height: {h}px; \
                 margin-left: auto; margin-right: auto; \
                 margin-bottom: {gap}px;",
                w   = props.w,
                h   = props.h,
                gap = props.gap_px,
            ),
            // element_coordinates() gives coords relative to this div's top-left,
            // which exactly equals page-local CSS pixels — no origin math needed.
            onmousedown: move |evt: MouseEvent| {
                let e = evt.element_coordinates();
                let x_pt = e.x as f32 * (72.0 / 96.0);
                let y_pt = e.y as f32 * (72.0 / 96.0);
                on_tile_click.call((page_index, x_pt, y_pt));
            },
            canvas {
                "src": "{canvas_id}",
                "data-cursor": "{data_cursor}",
                style: format!(
                    "width: {w}px; height: {h}px; display: block;",
                    w = props.w,
                    h = props.h,
                ),
            }
        }
    }
}
