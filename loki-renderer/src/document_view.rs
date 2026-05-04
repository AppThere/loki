// Copyright 2026 AppThere Loki contributors
// SPDX-License-Identifier: Apache-2.0

//! DocumentView component for rendering pages from loki-renderer cache.

use std::sync::{Arc, Mutex};

use dioxus::native::use_wgpu;
use dioxus::prelude::*;
use loki_doc_model::document::Document;
use loki_render_cache::PageCache;
use loki_theme::tokens;

use crate::doc_page_source::DocPageSource;
use crate::page_paint_source::LokiPageSource;
use crate::renderer_state::RendererState;
use crate::scroll_driver::{on_scroll_event, use_settle_detector};

// ── DocumentViewProps ─────────────────────────────────────────────────────────

/// Props for the DocumentView component.
#[derive(Props, Clone)]
pub struct DocumentViewProps {
    pub doc: Arc<Document>,
    pub viewport_height_px: f64,
}

impl PartialEq for DocumentViewProps {
    fn eq(&self, _other: &Self) -> bool {
        false // Conservatively always re-render
    }
}

// ── PageTile ──────────────────────────────────────────────────────────────────

#[derive(Clone, Props)]
struct PageTileProps {
    /// Shared tier-and-dirty metadata store from `RendererState`.
    cache: Arc<Mutex<PageCache>>,
    /// Document layout + page-size source.
    source: Arc<DocPageSource>,
    page_index: usize,
    top: f64,
    w: f64,
    h: f64,
}

impl PartialEq for PageTileProps {
    fn eq(&self, other: &Self) -> bool {
        Arc::ptr_eq(&self.cache, &other.cache)
            && Arc::ptr_eq(&self.source, &other.source)
            && self.page_index == other.page_index
            && self.w == other.w
            && self.h == other.h
    }
}

/// A single page rendered into a Blitz GPU canvas.
///
/// Calls `use_wgpu` exactly once per instance — the hook-count invariant is
/// satisfied by Dioxus's key-based reconciliation in `DocumentView`.
#[allow(non_snake_case)]
fn PageTile(props: PageTileProps) -> Element {
    let cache = props.cache.clone();
    let source = props.source.clone();
    let page_index = props.page_index;

    let canvas_id = use_wgpu(move || LokiPageSource::new(cache, source, page_index));

    rsx! {
        div {
            style: format!(
                "position: absolute; top: {top}px; left: 50%; \
                 transform: translateX(-50%); width: {w}px; height: {h}px;",
                top = props.top,
                w   = props.w,
                h   = props.h,
            ),
            canvas {
                "src": "{canvas_id}",
                style: format!(
                    "width: {w}px; height: {h}px; display: block;",
                    w = props.w,
                    h = props.h,
                ),
            }
        }
    }
}

// ── DocumentView ──────────────────────────────────────────────────────────────

/// Root document rendering component.
///
/// - Initialises `RendererState` via `use_hook` and provides it as context.
/// - Launches the settle detector via `use_settle_detector`.
/// - Renders one `PageTile` per page; each tile registers a `LokiPageSource`
///   via `use_wgpu` and Blitz drives rendering each frame.
/// - Passes scroll events to `on_scroll_event`.
#[component]
pub fn DocumentView(props: DocumentViewProps) -> Element {
    let renderer = use_hook(|| {
        RendererState::new(props.doc.clone(), props.viewport_height_px)
    });
    provide_context(renderer.clone());

    let renderer_settle = renderer.clone();
    use_settle_detector(renderer.scroll, move || {
        renderer_settle.on_settle();
    });

    let scroll = renderer.scroll;
    let onscroll = move |evt: Event<ScrollData>| {
        on_scroll_event(scroll, evt.scroll_top());
    };

    let gen = renderer.source.current_generation();
    let layout_guard = renderer.source.layout_for_generation(gen);
    let mut total_height = 0.0f64;
    let pages: Vec<(usize, f64, f64, f64)> =
        if let Some((_, layout)) = layout_guard.as_ref() {
            layout
                .pages
                .iter()
                .enumerate()
                .map(|(i, p)| {
                    let h = p.page_size.height as f64;
                    let top = total_height;
                    total_height += h + tokens::PAGE_GAP_PX as f64;
                    (i, top, p.page_size.width as f64, h)
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

    rsx! {
        div {
            style: "width: 100%; height: 100%; overflow-y: auto;",
            onscroll: onscroll,
            div {
                style: "position: relative; width: 100%; height: {total_height}px;",
                for (idx, top, w, h) in pages {
                    PageTile {
                        key: "{idx}",
                        cache: renderer.cache.clone(),
                        source: renderer.source.clone(),
                        page_index: idx,
                        top,
                        w,
                        h,
                    }
                }
            }
        }
    }
}
