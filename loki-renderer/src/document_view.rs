// Copyright 2026 AppThere Loki contributors
// SPDX-License-Identifier: Apache-2.0

//! DocumentView component for rendering pages from loki-renderer cache.

use std::sync::{Arc, Mutex};

use appthere_canvas::{PageCache, PageIndex, ScrollState};
use appthere_ui::tokens;
#[cfg(not(target_os = "android"))]
use dioxus::native::use_wgpu;
use dioxus::prelude::*;
use loki_doc_model::document::Document;

use crate::doc_page_source::DocPageSource;
#[cfg(not(target_os = "android"))]
use crate::page_paint_source::LokiPageSource;
use crate::renderer_state::RendererState;
use crate::scroll_driver::{on_scroll_event, use_settle_detector};

// ── RendererCursorPos ─────────────────────────────────────────────────────────

/// Minimal cursor position for GPU painting. No Loro dependency.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct RendererCursorPos {
    pub page_index: usize,
    pub paragraph_index: usize,
    pub byte_offset: usize,
}

// ── DocumentViewProps ─────────────────────────────────────────────────────────

/// Props for the DocumentView component.
#[derive(Props, Clone)]
pub struct DocumentViewProps {
    pub doc: Arc<Document>,
    pub viewport_height_px: f64,
    pub cursor_pos: Option<RendererCursorPos>,
    /// Called with `(page_index, x_pt, y_pt)` in layout points when the user
    /// clicks a page tile. The caller performs the hit test and updates cursor state.
    pub on_tile_click: EventHandler<(usize, f32, f32)>,
}

impl PartialEq for DocumentViewProps {
    fn eq(&self, _other: &Self) -> bool {
        false // Conservatively always re-render
    }
}

// ── PageTile ──────────────────────────────────────────────────────────────────

#[derive(Clone, Props)]
struct PageTileProps {
    cache: Arc<Mutex<PageCache<PageIndex>>>,
    source: Arc<DocPageSource>,
    page_index: usize,
    w: f64,
    h: f64,
    shared_renderer: Arc<Mutex<Option<vello::Renderer>>>,
    cursor_holder: Arc<Mutex<Option<RendererCursorPos>>>,
    cursor_pos: Option<RendererCursorPos>,
    /// Called with `(page_index, x_pt, y_pt)` in layout points when the user
    /// clicks anywhere on this page tile. The parent uses this to call
    /// `hit_test_page` without needing window-relative origin math.
    on_tile_click: EventHandler<(usize, f32, f32)>,
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
        // on_tile_click intentionally excluded — EventHandler identity does not
        // affect render output; omitting it avoids spurious re-renders.
    }
}

/// A single page rendered into a Blitz GPU canvas.
#[cfg(not(target_os = "android"))]
#[allow(non_snake_case)]
fn PageTile(props: PageTileProps) -> Element {
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

    // A dummy data attribute that changes whenever the cursor moves, forcing
    // Blitz to mark the canvas dirty and call render() again.
    // COMPAT(dioxus-native): data-* attributes confirmed working in Blitz.
    let data_cursor = match props.cursor_pos {
        Some(cp) if cp.page_index == props.page_index => {
            format!("{}-{}", cp.paragraph_index, cp.byte_offset)
        }
        _ => String::new(),
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
                gap = tokens::PAGE_GAP_PX,
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

#[cfg(target_os = "android")]
#[allow(non_snake_case)]
fn PageTile(props: PageTileProps) -> Element {
    // COMPAT(dioxus-native): GPU canvas (use_wgpu/CustomPaintSource) unavailable
    // with the CPU renderer on Android. Pages render as placeholder blocks.
    rsx! {
        div {
            style: format!(
                "display: block; width: {w}px; height: {h}px; \
                 margin-left: auto; margin-right: auto; \
                 margin-bottom: {gap}px; background: #2a2a2a;",
                w = props.w,
                h = props.h,
                gap = tokens::PAGE_GAP_PX,
            ),
        }
    }
}

// ── DocumentView ──────────────────────────────────────────────────────────────

/// Root document rendering component.
#[component]
pub fn DocumentView(props: DocumentViewProps) -> Element {
    // use_signal must be called at the top level — not inside use_hook —
    // to avoid "hook list already borrowed: BorrowMutError".
    let scroll = use_signal(|| ScrollState::new(props.viewport_height_px));
    let renderer = use_hook(|| RendererState::new(props.doc.clone(), scroll));
    // Push the latest document into the page source on every render.
    // `update_doc` compares by Arc pointer and returns immediately when
    // the document has not changed since the last render, so this is
    // cheap between mutations.
    renderer.source.update_doc(props.doc.clone());
    provide_context(renderer.clone());

    // Shared cursor holder: written by PageTile on each render, read by
    // LokiPageSource during the GPU paint call.
    let cursor_holder: Arc<Mutex<Option<RendererCursorPos>>> =
        use_hook(|| Arc::new(Mutex::new(None)));

    let renderer_settle = renderer.clone();
    let (_task, _tx) = use_settle_detector(renderer.scroll, move || {
        renderer_settle.on_settle();
    });

    let scroll = renderer.scroll;
    let phase_tx = renderer.phase_tx.clone();
    let onscroll = move |evt: Event<ScrollData>| {
        on_scroll_event(scroll, evt.scroll_top(), &phase_tx);
    };

    // loki-layout page dimensions are in typographic points (1pt = 1/72 inch).
    // CSS pixels assume 96dpi. Conversion: 1pt = 96/72 CSS px.
    const PTS_TO_CSS_PX: f64 = 96.0 / 72.0;

    let doc_gen = renderer.source.current_generation();
    let layout_guard = renderer.source.layout_for_generation(doc_gen);
    let mut total_height = 0.0f64;
    let pages: Vec<(usize, f64, f64)> = if let Some((_, layout)) = layout_guard.as_ref() {
        layout
            .pages
            .iter()
            .enumerate()
            .map(|(i, p)| {
                let h = p.page_size.height as f64 * PTS_TO_CSS_PX;
                total_height += h + tokens::PAGE_GAP_PX as f64;
                (i, p.page_size.width as f64 * PTS_TO_CSS_PX, h)
            })
            .collect()
    } else {
        vec![]
    };
    drop(layout_guard);

    let (hot, warm, cold) = renderer
        .cache
        .lock()
        .map(|g| g.page_count_by_tier())
        .unwrap_or((0, 0, 0));
    tracing::debug!(hot, warm, cold, "DocumentView rendered");

    let cursor_pos = props.cursor_pos;
    let on_tile_click = props.on_tile_click;

    rsx! {
        div {
            // No overflow-y: auto here — scrolling is owned by the parent
            // container in editor_canvas.rs.  DocumentView is a non-scrolling
            // block that fills its parent's content area.
            style: "width: 100%; height: 100%;",
            onscroll: onscroll,
            div {
                // Block flow: height determined by stacked page tiles.
                // position:relative is retained so future absolutely-positioned
                // overlays (cursor, selection) have a containing block.
                style: "position: relative; width: 100%;",
                for (idx, w, h) in pages {
                    PageTile {
                        key: "{idx}",
                        cache: renderer.cache.clone(),
                        source: renderer.source.clone(),
                        page_index: idx,
                        w,
                        h,
                        shared_renderer: renderer.shared_renderer.clone(),
                        cursor_holder: cursor_holder.clone(),
                        cursor_pos,
                        on_tile_click,
                    }
                }
            }
        }
    }
}
