// Copyright 2026 AppThere Loki contributors
// SPDX-License-Identifier: Apache-2.0

//! DocumentView component for rendering pages from loki-renderer cache.

use std::sync::Arc;

use dioxus::prelude::*;
use loki_doc_model::document::Document;
use loki_render_cache::{CacheTier, PageIndex};
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
    let mut total_height = 0.0;
    
    let mut hot_count = 0;
    let mut warm_count = 0;
    let mut cold_count = 0;

    let cache_guard = renderer.cache.lock().unwrap();

    let pages = layout.pages.iter().enumerate().map(|(i, p)| {
        let h = p.page_size.height as f64;
        let top = total_height;
        total_height += h + tokens::PAGE_GAP_PX as f64;
        
        let index = PageIndex(i as u32);
        let cached = cache_guard.get(index);
        
        let tier = match cached {
            Some(cp) => {
                match cp.tier {
                    CacheTier::Hot => hot_count += 1,
                    CacheTier::Warm => warm_count += 1,
                    CacheTier::Cold => cold_count += 1,
                }
                Some(cp.tier)
            },
            None => None
        };
        
        (index, top, p.page_size.width as f64, h, tier)
    }).collect::<Vec<_>>();

    tracing::debug!(
        hot = hot_count,
        warm = warm_count,
        cold = cold_count,
        "Document view rendered"
    );

    rsx! {
        div {
            style: "width: 100%; height: 100%; overflow-y: auto;",
            onscroll: onscroll,
            div {
                style: "position: relative; width: 100%; height: {total_height}px;",
                for (idx, top, w, h, tier) in pages {
                    if matches!(tier, Some(CacheTier::Hot) | Some(CacheTier::Warm)) {
                        div {
                            key: "{idx.0}",
                            style: format!(
                                "position: absolute; top: {top}px; left: 50%; \
                                 transform: translateX(-50%); width: {w}px; height: {h}px; \
                                 background: {bg}; display: flex; align-items: center; justify-content: center;",
                                bg = tokens::COLOR_SURFACE_PAGE,
                            ),
                            span { "Page {idx.0} ({tier:?})" }
                        }
                    } else {
                        div {
                            key: "{idx.0}",
                            style: format!(
                                "position: absolute; top: {top}px; left: 50%; \
                                 transform: translateX(-50%); width: {w}px; height: {h}px; \
                                 background: #f0f0f0; display: flex; align-items: center; justify-content: center;"
                            ),
                            span { "Page {idx.0} (Cold/None)" }
                        }
                    }
                }
            }
        }
    }
}
