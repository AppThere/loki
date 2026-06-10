// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 AppThere Loki contributors

//! DocumentView component for rendering pages from loki-renderer cache.

use std::sync::{Arc, Mutex};

use appthere_canvas::ScrollState;
#[cfg(any(not(target_os = "android"), android_gpu))]
use appthere_canvas::{PageCache, PageIndex};
#[cfg(any(not(target_os = "android"), android_gpu))]
use appthere_ui::tokens;
// use_wgpu and LokiPageSource are enabled on: desktop, and Android devices
// built with RUSTFLAGS='--cfg android_gpu' (Vulkan-capable physical devices).
// The Android emulator uses SwiftShader which lacks Vello's compute pipeline,
// so it falls through to the CPU-renderer placeholder path below.
#[cfg(any(not(target_os = "android"), android_gpu))]
use dioxus::native::use_wgpu;
use dioxus::prelude::*;
use loki_doc_model::document::Document;

#[cfg(any(not(target_os = "android"), android_gpu))]
use crate::doc_page_source::DocPageSource;
#[cfg(any(not(target_os = "android"), android_gpu))]
use crate::page_paint_source::LokiPageSource;
use crate::renderer_state::RendererState;
use crate::scroll_driver::{on_scroll_event, use_settle_detector};

#[cfg(all(target_os = "android", not(android_gpu)))]
use crate::page_tile_cpu::CpuDocView;

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

#[cfg(any(not(target_os = "android"), android_gpu))]
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
    /// Called with `(page_index, x_pt, y_pt)` in layout points when the user
    /// clicks anywhere on this page tile. The parent uses this to call
    /// `hit_test_page` without needing window-relative origin math.
    pub(crate) on_tile_click: EventHandler<(usize, f32, f32)>,
}

#[cfg(any(not(target_os = "android"), android_gpu))]
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
        // on_tile_click intentionally excluded — EventHandler identity does not
        // affect render output; omitting it avoids spurious re-renders.
    }
}

/// A single page rendered into a Blitz GPU canvas.
#[cfg(any(not(target_os = "android"), android_gpu))]
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

// ── DocumentView ──────────────────────────────────────────────────────────────

/// Root document rendering component.
#[component]
pub fn DocumentView(props: DocumentViewProps) -> Element {
    // use_signal must be called at the top level — not inside use_hook —
    // to avoid "hook list already borrowed: BorrowMutError".
    let scroll = use_signal(|| ScrollState::new(props.viewport_height_px));
    // Bumped by on_settle after each retier; read below so a settle forces a
    // re-render that repaints demoted tiles at their new resolution.
    let settle_epoch = use_signal(|| 0u64);
    let renderer = use_hook(|| RendererState::new(props.doc.clone(), scroll, settle_epoch));
    // Push the latest document into the page source on every render.
    // `update_doc` compares by Arc pointer and returns immediately when
    // the document has not changed since the last render, so this is
    // cheap between mutations.
    renderer.source.update_doc(props.doc.clone());
    provide_context(renderer.clone());

    // Shared cursor holder: written by PageTile on each render, read by
    // LokiPageSource during the GPU paint call.  Declared on all paths to
    // keep hook indices stable; the CPU path uses an _ prefix to suppress
    // the unused-variable lint.
    #[cfg(any(not(target_os = "android"), android_gpu))]
    let cursor_holder: Arc<Mutex<Option<RendererCursorPos>>> =
        use_hook(|| Arc::new(Mutex::new(None)));
    #[cfg(all(target_os = "android", not(android_gpu)))]
    let _cursor_holder: Arc<Mutex<Option<RendererCursorPos>>> =
        use_hook(|| Arc::new(Mutex::new(None)));

    let renderer_settle = renderer.clone();
    use_settle_detector(&renderer.phase_tx, move || {
        renderer_settle.on_settle();
    });

    let scroll = renderer.scroll;
    let phase_tx = renderer.phase_tx.clone();
    let onscroll = move |evt: Event<ScrollData>| {
        on_scroll_event(scroll, evt.scroll_top(), &phase_tx);
    };

    let doc_gen = renderer.source.current_generation();

    // ── Android CPU: flat web-style renderer ─────────────────────────────────
    // All hooks have been called above; early return is safe.
    #[cfg(all(target_os = "android", not(android_gpu)))]
    return rsx! {
        div {
            style: "width: 100%; height: 100%;",
            onscroll: onscroll,
            CpuDocView { source: renderer.source.clone(), doc_gen }
        }
    };

    // ── GPU / desktop: paged tile renderer ───────────────────────────────────
    #[cfg(any(not(target_os = "android"), android_gpu))]
    {
        const PTS_TO_CSS_PX: f64 = 96.0 / 72.0;
        let layout_guard = renderer.source.layout_for_generation(doc_gen);
        let pages: Vec<(usize, f64, f64)> = if let Some((_, layout)) = layout_guard.as_ref() {
            layout
                .pages
                .iter()
                .enumerate()
                .map(|(i, p)| {
                    let h = p.page_size.height as f64 * PTS_TO_CSS_PX;
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
        // Read (and subscribe to) the settle epoch so a scroll-settle retier
        // re-renders this component and repaints demoted tiles.
        let epoch = settle_epoch();

        return rsx! {
            div {
                style: "width: 100%; height: 100%;",
                onscroll: onscroll,
                div {
                    style: format!(
                        "position: relative; width: 100%; padding-bottom: {pb}px;",
                        pb = tokens::SPACE_6,
                    ),
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
                            doc_gen,
                            settle_epoch: epoch,
                            on_tile_click,
                        }
                    }
                }
            }
        };
    }
}
