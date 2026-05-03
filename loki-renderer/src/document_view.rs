// Copyright 2026 AppThere Loki contributors
// SPDX-License-Identifier: Apache-2.0

//! DocumentView component for rendering pages from loki-renderer cache.

use std::sync::Arc;

use dioxus::prelude::*;
use loki_doc_model::document::Document;
use loki_render_cache::PageIndex;
use loki_theme::tokens;

use crate::renderer_state::RendererState;
use crate::scroll_driver::{on_scroll_event, use_settle_detector};

/// Props for the DocumentView component.
#[derive(Props, Clone)]
pub struct DocumentViewProps {
    pub doc: Arc<Document>,
    pub viewport_height_px: f64,
    pub device: Arc<wgpu::Device>,
    pub wgpu_queue: Arc<wgpu::Queue>,
}

impl PartialEq for DocumentViewProps {
    fn eq(&self, _other: &Self) -> bool {
        false // Conservatively always re-render, Document doesn't implement PartialEq
    }
}

/// Snapshot of one page's display state, built under the cache lock and used
/// after the lock is released.
struct PageSnapshot {
    index: PageIndex,
    top: f64,
    w: f64,
    h: f64,
    /// PNG data URI ready for `img.src`, or `None` while the page is still
    /// rendering or has never been settled.
    data_uri: Option<Arc<String>>,
}

/// Root document rendering component.
///
/// - Initialises `RendererState` via `use_hook` and provides it as context.
/// - Launches the settle detector via `use_settle_detector`.
/// - Renders visible page tiles from the cache.
/// - Passes scroll events to `on_scroll_event`.
#[component]
pub fn DocumentView(props: DocumentViewProps) -> Element {
    let renderer = use_hook(|| {
        RendererState::new(
            props.doc.clone(),
            props.viewport_height_px,
            props.device.clone(),
            props.wgpu_queue.clone(),
        )
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

    let layout = renderer.source.layout();

    // Snapshot cache state into a Vec, then drop the guard before rsx! so the
    // render thread does not hold the mutex across the Dioxus diffing pass.
    let (pages, total_height) = {
        let cache = renderer.cache.lock().unwrap_or_else(|e| e.into_inner());
        let mut top = 0.0f64;
        let mut hot = 0u32;
        let mut warm = 0u32;
        let mut cold = 0u32;

        let snaps: Vec<PageSnapshot> = layout
            .pages
            .iter()
            .enumerate()
            .map(|(i, p)| {
                let h = p.page_size.height as f64;
                let page_top = top;
                top += h + tokens::PAGE_GAP_PX as f64;

                let index = PageIndex(i as u32);
                let cached = cache.get(index);
                let data_uri = cached.and_then(|cp| {
                    match cp.tier {
                        loki_render_cache::CacheTier::Hot => hot += 1,
                        loki_render_cache::CacheTier::Warm => warm += 1,
                        loki_render_cache::CacheTier::Cold => cold += 1,
                    }
                    cp.data_uri.clone()
                });

                PageSnapshot {
                    index,
                    top: page_top,
                    w: p.page_size.width as f64,
                    h,
                    data_uri,
                }
            })
            .collect();

        tracing::debug!(hot, warm, cold, "Document view rendered");
        (snaps, top)
    };
    // Cache guard dropped here — rsx! runs without holding the mutex.

    rsx! {
        div {
            style: "width: 100%; height: 100%; overflow-y: auto;",
            onscroll: onscroll,
            div {
                style: "position: relative; width: 100%; height: {total_height}px;",
                for snap in pages {
                    if let Some(ref uri) = snap.data_uri {
                        div {
                            key: "{snap.index.0}",
                            style: format!(
                                "position: absolute; top: {top}px; left: 50%; \
                                 transform: translateX(-50%); width: {w}px; height: {h}px;",
                                top = snap.top,
                                w = snap.w,
                                h = snap.h,
                            ),
                            img {
                                src: "{uri}",
                                width: "{snap.w}",
                                height: "{snap.h}",
                                style: "display: block; width: 100%; height: 100%;",
                            }
                        }
                    } else {
                        div {
                            key: "{snap.index.0}",
                            style: format!(
                                "position: absolute; top: {top}px; left: 50%; \
                                 transform: translateX(-50%); width: {w}px; height: {h}px; \
                                 background: {bg}; display: flex; \
                                 align-items: center; justify-content: center;",
                                top = snap.top,
                                w = snap.w,
                                h = snap.h,
                                bg = tokens::COLOR_SURFACE_PAGE,
                            ),
                        }
                    }
                }
            }
        }
    }
}
