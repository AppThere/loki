// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 AppThere Loki contributors

//! DocumentView component for rendering pages from loki-renderer cache.

use std::sync::{Arc, Mutex};

#[cfg(any(not(target_os = "android"), android_gpu))]
use appthere_ui::tokens;
use dioxus::prelude::*;
use loki_doc_model::document::Document;
use loki_layout::PaginatedLayout;

// PageTile (and the wgpu paint path under it) is enabled on: desktop, and
// Android devices built with RUSTFLAGS='--cfg android_gpu' (Vulkan-capable
// physical devices). The Android emulator uses SwiftShader which lacks Vello's
// compute pipeline, so it falls through to the CPU-renderer path below.
#[cfg(any(not(target_os = "android"), android_gpu))]
use crate::page_tile::PageTile;
#[cfg(any(not(target_os = "android"), android_gpu))]
use crate::render_layout::RenderMode;
use crate::renderer_state::RendererState;

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

/// Caret + optional range selection for GPU painting. `anchor == focus` (by
/// paragraph/byte) means a collapsed caret with no selection.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct RendererSelection {
    pub focus: RendererCursorPos,
    pub anchor: RendererCursorPos,
}

impl RendererSelection {
    /// True when there is no range selection (anchor and focus coincide).
    pub fn is_collapsed(&self) -> bool {
        self.anchor.paragraph_index == self.focus.paragraph_index
            && self.anchor.byte_offset == self.focus.byte_offset
    }
}

/// A right-click on a page tile, carrying both the tile-local layout-point
/// coordinates (for an accurate hit test) and the window-relative client
/// coordinates (to anchor a floating menu at the cursor).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct TileContext {
    /// Index of the page tile that was right-clicked.
    pub page_index: usize,
    /// X within the tile, in layout points (from `element_coordinates`).
    pub x_pt: f32,
    /// Y within the tile, in layout points.
    pub y_pt: f32,
    /// Window-relative X of the cursor, in CSS pixels.
    pub client_x: f32,
    /// Window-relative Y of the cursor, in CSS pixels.
    pub client_y: f32,
}

// ── DocumentViewProps ─────────────────────────────────────────────────────────

/// Props for the DocumentView component.
#[derive(Props, Clone)]
pub struct DocumentViewProps {
    pub doc: Arc<Document>,
    /// Paginated layout already computed by the editor for `doc`, reused in
    /// paginated mode so the renderer does not lay the document out a second
    /// time (the single canonical layout). `None` falls back to computing it
    /// on the render path (e.g. before the first editor layout, or on the
    /// Android CPU reflow path).
    pub paginated_layout: Option<Arc<PaginatedLayout>>,
    pub viewport_height_px: f64,
    /// Current vertical scroll offset of the editor's scroll container, in CSS
    /// px. Drives tile virtualization: only pages within ~one screen of this
    /// offset are GPU-rendered. The editor owns the scroll container (this
    /// component is laid out inside it), so the scroll position must be passed
    /// in — the renderer's own scroll signal is not updated by the real scroll.
    pub viewport_top_px: f64,
    /// The caret / selection focus position.
    pub cursor_pos: Option<RendererCursorPos>,
    /// The selection anchor. When it differs from `cursor_pos`, a range
    /// selection is highlighted between them (reflow mode).
    pub selection_anchor: Option<RendererCursorPos>,
    /// Current layout mode. Ignored on the Android CPU path, which only supports
    /// [`ViewMode::Reflow`].
    pub view_mode: ViewMode,
    /// Available viewport width in CSS pixels for [`ViewMode::Reflow`].
    /// `<= 0` means "not yet measured" — the view falls back to paginated
    /// rendering until a real width arrives.
    pub reflow_width_px: f64,
    /// Called with `(page_index, x_pt, y_pt)` in layout points when the user
    /// clicks a page tile in **paginated** mode. The caller performs the hit test
    /// and updates cursor state.
    pub on_tile_click: EventHandler<(usize, f32, f32)>,
    /// Called with `(block_index, byte_offset)` when the user clicks in
    /// **reflow** mode. This component owns the reflow layout, so it hit-tests
    /// the click itself and reports the resolved document position.
    pub on_reflow_click: EventHandler<(usize, usize)>,
    /// Called with `(block_index, byte_offset)` while drag-selecting in
    /// **reflow** mode (mouse moved with a button held). The caller extends the
    /// selection focus to this position.
    pub on_reflow_drag: EventHandler<(usize, usize)>,
    /// Called when a page tile is right-clicked (paginated mode), carrying the
    /// accurate tile-local + client coordinates. Drives the spelling context menu.
    pub on_tile_context: EventHandler<TileContext>,
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
    let renderer = use_hook(|| RendererState::new(props.doc.clone()));
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
    let cursor_holder: Arc<Mutex<Option<RendererSelection>>> =
        use_hook(|| Arc::new(Mutex::new(None)));
    #[cfg(all(target_os = "android", not(android_gpu)))]
    let _cursor_holder: Arc<Mutex<Option<RendererSelection>>> =
        use_hook(|| Arc::new(Mutex::new(None)));

    // ── Android CPU: flat web-style renderer ─────────────────────────────────
    // All hooks have been called above; early return is safe.
    #[cfg(all(target_os = "android", not(android_gpu)))]
    {
        let doc_gen = renderer.source.current_generation();
        return rsx! {
            div {
                style: "width: 100%; height: 100%;",
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
        // Single canonical layout: in paginated mode reuse the layout the editor
        // already computed for this document instead of laying it out again.
        // Provided after set_render_mode so it is keyed to the current
        // generation; reflow mode computes its own width-dependent layout.
        if render_mode == RenderMode::Paginated
            && let Some(layout) = props.paginated_layout.clone()
        {
            renderer.source.provide_paginated_layout(layout);
        }
        let doc_gen = renderer.source.current_generation();

        // Tile virtualization bounds rendered pages to the viewport
        // neighbourhood, and every mounted tile renders at full resolution
        // (see LokiPageSource) — there is no resolution-tiering cache.

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

        tracing::debug!(is_reflow, "DocumentView rendered");

        // Caret + selection flow through for both modes; LokiPageSource paints
        // them via the page editing data (paginated) or the continuous editing
        // data (reflow). In reflow, rewrite the focus `page_index` to the band
        // tile that actually holds the caret so that tile is invalidated as the
        // caret moves.
        let to_band = |cp: RendererCursorPos| {
            if is_reflow {
                let band = renderer
                    .source
                    .reflow_cursor_band(cp.paragraph_index, cp.byte_offset)
                    .unwrap_or(0);
                RendererCursorPos {
                    page_index: band,
                    ..cp
                }
            } else {
                cp
            }
        };
        let selection = props.cursor_pos.map(|focus| {
            let focus = to_band(focus);
            RendererSelection {
                anchor: props.selection_anchor.map(to_band).unwrap_or(focus),
                focus,
            }
        });
        let gap_px = if is_reflow {
            0.0
        } else {
            tokens::PAGE_GAP_PX as f64
        };
        let on_tile_click = props.on_tile_click;
        let on_reflow_click = props.on_reflow_click;
        let on_reflow_drag = props.on_reflow_drag;
        let on_tile_context = props.on_tile_context;

        // White backdrop behind reflow tiles so any hairline seam where two
        // zero-gap bands meet shows white (matching the page) rather than the
        // grey canvas. Paginated mode keeps the grey inter-page gutter.
        let wrapper_bg = if is_reflow {
            " background: #FFFFFF;"
        } else {
            ""
        };

        // ── Viewport virtualization ──────────────────────────────────────────
        // Only pages near the viewport get a GPU tile; the rest render as cheap
        // page-sized placeholders. This bounds first-paint (and texture memory)
        // to the visible neighbourhood instead of GPU-painting every page of the
        // document up front — the dominant open-latency cost. The window is the
        // visible range grown by one screen on each side, so a scroll reveals an
        // already-painted tile. Reading the scroll offset here subscribes this
        // component to scroll, so the window follows the viewport; unchanged
        // tiles are skipped by `PageTile`'s `PartialEq`, so scrolling repaints
        // nothing on the GPU until a new page enters the window.
        let heights: Vec<f64> = pages.iter().map(|&(_, _, h)| h).collect();
        let visibility = crate::virtualize::visible_window(
            &heights,
            gap_px,
            props.viewport_top_px,
            props.viewport_height_px,
        );
        let tiles: Vec<(usize, f64, f64, bool)> = pages
            .iter()
            .zip(visibility)
            .map(|(&(idx, w, h), visible)| (idx, w, h, visible))
            .collect();

        return rsx! {
            div {
                style: "width: 100%; height: 100%;",
                // PATCH(loki): this root mounts when the document content first
                // appears inside the editor's scroll container — typically after
                // an async load, replacing a one-page loading placeholder. The
                // scroll container itself does not re-mount, so without an
                // `onmounted` somewhere in this freshly-mounted subtree the shell
                // never re-runs `resync_scroll_geometry` (dioxus-native
                // `flush_mounted` only resyncs when an `onmounted` listener is
                // pending). That leaves the container's Taffy scroll overflow
                // stale at the placeholder's ~one-page height: the wheel sees a
                // non-scrollable container (does nothing until a mouse-move forces
                // a re-resolve) and the scrollbar thumb is sized for one page.
                // The handler is intentionally empty — its mere presence makes the
                // shell resolve layout and re-dispatch scroll geometry the moment
                // the document mounts.
                onmounted: move |_| {},
                div {
                    style: format!(
                        "position: relative; width: 100%; padding-bottom: {pb}px;{bg}",
                        pb = tokens::SPACE_6,
                        bg = wrapper_bg,
                    ),
                    for (idx, w, h, visible) in tiles {
                        if visible {
                            PageTile {
                                key: "{idx}",
                                source: renderer.source.clone(),
                                page_index: idx,
                                w,
                                h,
                                shared_renderer: renderer.shared_renderer.clone(),
                                cursor_holder: cursor_holder.clone(),
                                selection,
                                doc_gen,
                                gap_px,
                                // In reflow, hit-test the click here (this component
                                // owns the reflow layout) and report the resolved
                                // (paragraph, byte). In paginated, forward the raw
                                // tile coordinates for the editor to hit-test.
                                on_tile_click: {
                                    let source = renderer.source.clone();
                                    move |(i, x, y): (usize, f32, f32)| {
                                        if is_reflow {
                                            if let Some((para, byte)) =
                                                source.reflow_hit_test(i, x, y)
                                            {
                                                on_reflow_click.call((para, byte));
                                            }
                                        } else {
                                            on_tile_click.call((i, x, y));
                                        }
                                    }
                                },
                                // Right-click → spelling context menu (paginated).
                                on_tile_context,
                                // Drag-select: reflow only (paginated drag is handled
                                // at the scroll-container level by the editor).
                                on_tile_drag: {
                                    let source = renderer.source.clone();
                                    move |(i, x, y): (usize, f32, f32)| {
                                        if is_reflow
                                            && let Some((para, byte)) =
                                                source.reflow_hit_test(i, x, y)
                                        {
                                            on_reflow_drag.call((para, byte));
                                        }
                                    }
                                },
                            }
                        } else {
                            // Off-window placeholder: same box as the tile (so the
                            // scroll geometry and scrollbar are unchanged), painted
                            // as a blank page. Becomes a real tile when scrolled near.
                            div {
                                key: "{idx}",
                                style: format!(
                                    "display: block; width: {w}px; height: {h}px; \
                                     margin-left: auto; margin-right: auto; \
                                     margin-bottom: {gap}px; background: #FFFFFF;",
                                    gap = gap_px,
                                ),
                            }
                        }
                    }
                }
            }
        };
    }
}
