// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 AppThere Loki contributors

//! [`PageTile`] — one render tile (a paginated page or a reflow band) painted
//! into a Blitz GPU canvas.  Split from `document_view.rs` to stay under the
//! file-size ceiling.

use std::sync::{Arc, Mutex};

use dioxus::html::input_data::MouseButton;
use dioxus::native::use_wgpu;
use dioxus::prelude::*;

use crate::doc_page_source::DocPageSource;
use crate::document_view::{RendererSelection, TileContext};
use crate::page_paint_source::LokiPageSource;

#[derive(Clone, Props)]
pub(crate) struct PageTileProps {
    pub(crate) source: Arc<DocPageSource>,
    pub(crate) page_index: usize,
    pub(crate) w: f64,
    pub(crate) h: f64,
    pub(crate) shared_renderer: Arc<Mutex<Option<vello::Renderer>>>,
    pub(crate) cursor_holder: Arc<Mutex<Option<RendererSelection>>>,
    pub(crate) selection: Option<RendererSelection>,
    /// Document generation — incremented on every mutation so that style
    /// changes that don't move the cursor still dirty the canvas.
    pub(crate) doc_gen: u64,
    /// Vertical gap below this tile in CSS px — the page gap in paginated
    /// mode, `0` in reflow mode so bands stitch into a continuous flow.
    pub(crate) gap_px: f64,
    /// Called with `(page_index, x_pt, y_pt)` in layout points on mouse-down.
    pub(crate) on_tile_click: EventHandler<(usize, f32, f32)>,
    /// Called with `(page_index, x_pt, y_pt)` on mouse-move while a button is
    /// held (drag-select). No-op for the paginated path (handled at the
    /// container level).
    pub(crate) on_tile_drag: EventHandler<(usize, f32, f32)>,
    /// Called on right-click (secondary button), carrying tile-local + client
    /// coordinates. Drives the spelling context menu.
    pub(crate) on_tile_context: EventHandler<TileContext>,
}

impl PartialEq for PageTileProps {
    fn eq(&self, other: &Self) -> bool {
        Arc::ptr_eq(&self.source, &other.source)
            && self.page_index == other.page_index
            && self.w == other.w
            && self.h == other.h
            && Arc::ptr_eq(&self.cursor_holder, &other.cursor_holder)
            && self.selection == other.selection
            && self.doc_gen == other.doc_gen
            && self.gap_px == other.gap_px
        // event handlers intentionally excluded — identity does not affect
        // render output; omitting them avoids spurious re-renders.
    }
}

/// A single page rendered into a Blitz GPU canvas.
#[allow(non_snake_case)]
pub(crate) fn PageTile(props: PageTileProps) -> Element {
    let source = props.source.clone();
    let page_index = props.page_index;
    let shared_renderer = props.shared_renderer.clone();
    let cursor_holder = props.cursor_holder.clone();
    let cursor_holder_wgpu = props.cursor_holder.clone();
    let on_tile_click = props.on_tile_click;
    let on_tile_drag = props.on_tile_drag;
    let on_tile_context = props.on_tile_context;

    // Write the current selection to the shared holder on every render so
    // LokiPageSource can read it during the GPU paint call.
    if let Ok(mut guard) = cursor_holder.lock() {
        *guard = props.selection;
    }

    let canvas_id = use_wgpu(move || {
        LokiPageSource::new(source, page_index, shared_renderer, cursor_holder_wgpu)
    });

    // Marks the canvas dirty (so Blitz re-invokes the paint source) on caret,
    // selection, or document changes. When a range selection is active it is
    // encoded into *every* tile's key — a selection can span bands, so all of
    // them must repaint as it changes. A collapsed caret only dirties the tile
    // it sits on, keeping plain caret movement cheap.
    // COMPAT(dioxus-native): data-* attributes confirmed working in Blitz.
    let data_cursor = match props.selection {
        Some(sel) if !sel.is_collapsed() => format!(
            "s{}-{}-{}-{}-{}",
            sel.anchor.paragraph_index,
            sel.anchor.byte_offset,
            sel.focus.paragraph_index,
            sel.focus.byte_offset,
            props.doc_gen,
        ),
        Some(sel) if sel.focus.page_index == props.page_index => format!(
            "{}-{}-{}",
            sel.focus.paragraph_index, sel.focus.byte_offset, props.doc_gen
        ),
        _ => format!("{}", props.doc_gen),
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
                // Right-click → context menu (carry client coords to anchor it);
                // any other button → cursor placement / drag origin.
                if evt.trigger_button() == Some(MouseButton::Secondary) {
                    let c = evt.client_coordinates();
                    on_tile_context.call(TileContext {
                        page_index,
                        x_pt,
                        y_pt,
                        client_x: c.x as f32,
                        client_y: c.y as f32,
                    });
                } else {
                    on_tile_click.call((page_index, x_pt, y_pt));
                }
            },
            // Drag-select: while a button is held, extend the selection to the
            // pointer. Tile-local coordinates again avoid origin math.
            onmousemove: move |evt: MouseEvent| {
                if evt.held_buttons().is_empty() {
                    return;
                }
                let e = evt.element_coordinates();
                let x_pt = e.x as f32 * (72.0 / 96.0);
                let y_pt = e.y as f32 * (72.0 / 96.0);
                on_tile_drag.call((page_index, x_pt, y_pt));
            },
            canvas {
                "src": "{canvas_id}",
                "data-cursor": "{data_cursor}",
                // I-beam over the page so the document reads as editable text.
                // Blitz resolves the cursor from the hovered node's computed
                // `cursor` and only updates it when the hovered node changes
                // (blitz-dom get_cursor / hover dispatch). A page is a single
                // canvas node, so the cursor cannot vary by position within it
                // (e.g. body vs. margin) without splitting the page into multiple
                // nodes — which would break full-page GPU painting. The page
                // therefore shows the text cursor uniformly; the surrounding grey
                // canvas background and inter-page gaps are separate nodes and
                // keep the default arrow.
                style: format!(
                    "width: {w}px; height: {h}px; display: block; cursor: text;",
                    w = props.w,
                    h = props.h,
                ),
            }
        }
    }
}
