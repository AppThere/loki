// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 AppThere Loki contributors

//! DocumentView component for rendering pages from loki-renderer cache.

use std::sync::{Arc, Mutex};

// Must stay unconditional: this module is ungated in `lib.rs` and BOTH arms of
// `DocumentView` need the prelude. Gating it to the GPU path broke the Android
// CPU target while desktop stayed green — gate the module, never this import.
use dioxus::prelude::*;

// PageTile (and the wgpu paint path under it) is enabled on: desktop, and
// Android devices built with RUSTFLAGS='--cfg android_gpu' (Vulkan-capable
// physical devices). The Android emulator (SwiftShader) and iOS Simulator both
// lack Vello's compute pipeline, so they fall through to the CPU-renderer path.
#[cfg(not(any(
    all(target_os = "android", not(android_gpu)),
    all(target_os = "ios", target_abi = "sim"),
)))]
use crate::page_tile::PageTile;
use crate::renderer_state::RendererState;

// The HTML-flow fallback is used on the CPU-renderer path: Android emulator and
// iOS Simulator. GPU targets render reflow mode through the real layout engine
// (RenderMode::Reflow — full font/size/alignment fidelity).
#[cfg(any(
    all(target_os = "android", not(android_gpu)),
    all(target_os = "ios", target_abi = "sim"),
))]
use crate::reflow_view::ReflowDocView;

pub use crate::view_types::{
    DocumentViewProps, RendererCursorPos, RendererSelection, TileContext, ViewMode,
};

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
    #[cfg(not(any(
        all(target_os = "android", not(android_gpu)),
        all(target_os = "ios", target_abi = "sim"),
    )))]
    let cursor_holder: Arc<Mutex<Option<RendererSelection>>> =
        use_hook(|| Arc::new(Mutex::new(None)));
    #[cfg(any(
        all(target_os = "android", not(android_gpu)),
        all(target_os = "ios", target_abi = "sim"),
    ))]
    let _cursor_holder: Arc<Mutex<Option<RendererSelection>>> =
        use_hook(|| Arc::new(Mutex::new(None)));

    // ── CPU path (Android emulator / iOS Simulator): flat web-style renderer ──
    // All hooks have been called above; early return is safe.
    #[cfg(any(
        all(target_os = "android", not(android_gpu)),
        all(target_os = "ios", target_abi = "sim"),
    ))]
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
    #[cfg(not(any(
        all(target_os = "android", not(android_gpu)),
        all(target_os = "ios", target_abi = "sim"),
    )))]
    {
        // Render mode, zoom, and the residency capability cap — one cluster,
        // because they must happen in that order and the read-back at the end
        // only means anything after the cap is applied. See `scale_resolve`.
        let zoom = crate::scale_resolve::resolve(
            &renderer.source,
            crate::scale_resolve::ScaleInputs {
                view_mode: props.view_mode,
                reflow_width_px: props.reflow_width_px,
                zoom: props.zoom,
                device_scale_factor: props.device_scale_factor,
                texture_budget: props.texture_budget,
            },
            // Single canonical layout: in paginated mode reuse the layout the
            // editor already computed instead of laying it out again. Handed to
            // `resolve` rather than seeded here, because it has to land after
            // `set_render_mode` and before the capability call reads page sizes —
            // an ordering that was stated in a comment and broken by the next
            // edit. See `scale_resolve::resolve`.
            props.paginated_layout.clone(),
        );
        let doc_gen = renderer.source.current_generation();

        // Tile boxes in CSS px at the current zoom, plus whether this is a
        // reflow layout. Both come off one layout guard (see `tile_plan`).
        let (pages, is_reflow) = crate::tile_plan::tile_boxes(&renderer.source, doc_gen, zoom);

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
        let gap_px = if is_reflow { 0.0 } else { props.page_gap_px };
        let on_tile_click = props.on_tile_click;
        let on_reflow_click = props.on_reflow_click;
        let on_open_link = props.on_open_link;
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
        // Spec 08 T2.1-T2.3: the window is now filtered by the texture budget as
        // well, and a surviving tile carries the rasterisation scale it may use.
        // The policy is `appthere_canvas::residency`'s, so this and the Phase 2
        // bench run the same code — a second implementation would drift, and a
        // drifted model measures itself.
        let tiles = crate::tile_plan::plan_tiles(
            &pages,
            gap_px,
            crate::tile_plan::ViewportInput {
                top_px: props.viewport_top_px,
                content_padding_top_px: f64::from(props.content_padding_top_px),
                height_px: props.viewport_height_px,
                zoom,
                device_scale_factor: props.device_scale_factor,
            },
            props.texture_budget,
        );

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
                        pb = props.content_padding_bottom_px,
                        bg = wrapper_bg,
                    ),
                    for tile in tiles {
                        if let Some(raster_scale) = tile.mount {
                            PageTile {
                                key: "{tile.index}",
                                source: renderer.source.clone(),
                                page_index: tile.index,
                                w: tile.w,
                                h: tile.h,
                                raster_scale,
                                zoom,
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
                                    move |(i, x, y, open_link): (usize, f32, f32, bool)| {
                                        if is_reflow {
                                            // Ctrl/Cmd+click over a link opens it instead
                                            // of placing the caret (feature 5.11); the
                                            // app supplies the actual opener.
                                            if open_link
                                                && let Some(url) = source.reflow_link_at(i, x, y)
                                            {
                                                on_open_link.call(url);
                                                return;
                                            }
                                            if let Some((para, byte)) =
                                                source.reflow_hit_test(i, x, y)
                                            {
                                                on_reflow_click.call((para, byte));
                                            }
                                        } else {
                                            on_tile_click.call((i, x, y, open_link));
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
                            // Placeholder: same box as the tile (so the scroll
                            // geometry and scrollbar are unchanged), painted as a
                            // blank page. Becomes a real tile when scrolled near —
                            // or when the texture budget has room for it again.
                            div {
                                key: "{tile.index}",
                                style: format!(
                                    "display: block; width: {w}px; height: {h}px; \
                                     margin-left: auto; margin-right: auto; \
                                     margin-bottom: {gap}px; background: #FFFFFF;",
                                    w = tile.w,
                                    h = tile.h,
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
