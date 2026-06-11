// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 AppThere Loki contributors

//! DocumentView component for rendering pages from loki-renderer cache.

use std::sync::{Arc, Mutex};

use appthere_canvas::ScrollState;
#[cfg(any(not(target_os = "android"), android_gpu))]
use appthere_ui::tokens;
use dioxus::prelude::*;
use loki_doc_model::document::Document;

// PageTile (and the wgpu paint path under it) is enabled on: desktop, and
// Android devices built with RUSTFLAGS='--cfg android_gpu' (Vulkan-capable
// physical devices). The Android emulator uses SwiftShader which lacks Vello's
// compute pipeline, so it falls through to the CPU-renderer path below.
#[cfg(any(not(target_os = "android"), android_gpu))]
use crate::page_tile::PageTile;
#[cfg(any(not(target_os = "android"), android_gpu))]
use crate::render_layout::RenderMode;
use crate::renderer_state::RendererState;
use crate::scroll_driver::{on_scroll_event, use_settle_detector};

// The HTML-flow fallback is only used on the Android CPU path; GPU targets
// render reflow mode through the real layout engine (RenderMode::Reflow).
#[cfg(all(target_os = "android", not(android_gpu)))]
use crate::reflow_view::ReflowDocView;

// ── ViewMode ──────────────────────────────────────────────────────────────────

/// How the document is laid out in the canvas.
#[derive(Clone, Copy, PartialEq, Eq, Debug, Default)]
pub enum ViewMode {
    /// Fixed print layout — one fixed-size page tile per page (needs the GPU
    /// paint path). This is the default on large viewports.
    #[default]
    Paginated,
    /// Reflowable, web-page-style continuous layout that wraps to the viewport
    /// width. The default on small viewports, and the only mode available on
    /// the Android CPU path.
    Reflow,
}

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
    /// Current layout mode. Ignored on the Android CPU path, which only supports
    /// [`ViewMode::Reflow`].
    pub view_mode: ViewMode,
    /// Available viewport width in CSS pixels for [`ViewMode::Reflow`].
    /// `<= 0` means "not yet measured" — the view falls back to paginated
    /// rendering until a real width arrives.
    pub reflow_width_px: f64,
    /// Called with `(page_index, x_pt, y_pt)` in layout points when the user
    /// clicks a page tile. The caller performs the hit test and updates cursor state.
    pub on_tile_click: EventHandler<(usize, f32, f32)>,
}

impl PartialEq for DocumentViewProps {
    fn eq(&self, _other: &Self) -> bool {
        false // Conservatively always re-render
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

    // ── Android CPU: flat web-style renderer ─────────────────────────────────
    // All hooks have been called above; early return is safe.
    #[cfg(all(target_os = "android", not(android_gpu)))]
    {
        let doc_gen = renderer.source.current_generation();
        return rsx! {
            div {
                style: "width: 100%; height: 100%;",
                onscroll: onscroll,
                ReflowDocView { source: renderer.source.clone(), doc_gen }
            }
        };
    }

    // ── GPU / desktop ─────────────────────────────────────────────────────────
    #[cfg(any(not(target_os = "android"), android_gpu))]
    {
        // Select the render mode before reading the generation: switching mode
        // (or a reflow width change) invalidates the layout cache and advances
        // the generation so every tile repaints against the new layout.
        // Reflow runs the real layout engine at the viewport width (full
        // formatting fidelity), presented as zero-gap virtual tiles.
        let render_mode = if props.view_mode == ViewMode::Reflow && props.reflow_width_px > 1.0 {
            RenderMode::Reflow {
                available_width_pt: (props.reflow_width_px * 72.0 / 96.0) as f32,
            }
        } else {
            RenderMode::Paginated
        };
        renderer.source.set_render_mode(render_mode);
        let doc_gen = renderer.source.current_generation();

        const PTS_TO_CSS_PX: f64 = 96.0 / 72.0;
        let layout_guard = renderer.source.layout_for_generation(doc_gen);
        let is_reflow = layout_guard
            .as_ref()
            .map(|(_, l)| l.is_reflow())
            .unwrap_or(false);
        let pages: Vec<(usize, f64, f64)> = if let Some((_, layout)) = layout_guard.as_ref() {
            (0..layout.page_count())
                .filter_map(|i| {
                    layout
                        .page_size_pts(i)
                        .map(|(w, h)| (i, w as f64 * PTS_TO_CSS_PX, h as f64 * PTS_TO_CSS_PX))
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
        tracing::debug!(hot, warm, cold, is_reflow, "DocumentView rendered");

        // Reflow layouts carry no per-page editing data, so the cursor cannot
        // be mapped to a tile — suppress it rather than painting it at a
        // paginated position that no longer exists on screen.
        let cursor_pos = if is_reflow { None } else { props.cursor_pos };
        let gap_px = if is_reflow {
            0.0
        } else {
            tokens::PAGE_GAP_PX as f64
        };
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
                            gap_px,
                            on_tile_click,
                        }
                    }
                }
            }
        };
    }
}
