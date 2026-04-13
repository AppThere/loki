// Copyright 2026 AppThere Loki contributors
// SPDX-License-Identifier: Apache-2.0

//! WGPU document canvas component.
//!
//! [`WgpuSurface`] is the integration point between the Dioxus Native UI tree
//! and the [`loki_vello`] GPU rendering pipeline.  For this scaffold session
//! the surface renders a blank white A4 page at fixed dimensions.  The WGPU
//! context acquisition and Vello scene submission are stubbed with clearly
//! marked `TODO` comments.
//!
//! # Integration seam
//!
//! Future work should replace the placeholder `div` with a native canvas
//! element acquired from Dioxus Native's windowing layer, then:
//!
//! 1. Obtain a `wgpu::Surface` from the window handle.
//! 2. Create (or reuse) a `vello::Renderer`.
//! 3. Build a `vello::Scene` and call `loki_vello::paint_layout(…)`.
//! 4. Submit via `vello::Renderer::render_to_surface(…)`.

use dioxus::prelude::*;

use crate::theme;

// ── ViewportRect ─────────────────────────────────────────────────────────────

/// Axis-aligned rectangle in document-space coordinates (CSS pixels at 1× scale).
///
/// Used to describe the currently visible portion of the document canvas for
/// the partial-rendering optimisation — see [`WgpuSurface`] future work.
#[derive(Clone, PartialEq, Debug)]
pub struct ViewportRect {
    /// Left edge in document-space pixels.
    pub x: f32,
    /// Top edge in document-space pixels.
    pub y: f32,
    /// Width in document-space pixels.
    pub width: f32,
    /// Height in document-space pixels.
    pub height: f32,
}

// ── WgpuSurface ───────────────────────────────────────────────────────────────

/// WGPU document canvas component.
///
/// Occupies the `flex: 1` region between the top and bottom toolbars.  The
/// component is statically sized to the document dimensions (A4 by default) and
/// centred in the available space.  Scroll behaviour is **not** implemented in
/// this session.
///
/// # Props
///
/// * `document_path` — serialised [`loki_file_access::FileAccessToken`] for the
///   document to render.  `None` renders a blank white page.
///
/// * `visible_rect` — see the **Future work** section below.
///
/// # Future work — `visible_rect`
///
/// When scroll infrastructure is wired up, `visible_rect` will be populated
/// with the viewport region in document-space coordinates by the parent
/// component.  The rendering backend should restrict the Vello scene to items
/// whose bounding boxes intersect `visible_rect`, avoiding GPU work proportional
/// to the full document size.  Concrete steps:
///
/// 1. Compute `visible_rect` from the scroll offset and canvas viewport size.
/// 2. Pass it to `loki_vello::paint_layout` as a clipping hint.
/// 3. Cull [`loki_layout::PositionedItem`]s that fall entirely outside the rect
///    before appending them to the Vello scene.
///
/// Until then, leave `visible_rect` as `None`.
#[component]
pub fn WgpuSurface(
    document_path: Option<String>,
    /// Currently visible portion of the document canvas.
    ///
    /// # Future work
    ///
    /// Populate with the current scroll viewport in document-space coordinates.
    /// The renderer should cull items outside this rect before building the
    /// Vello scene, reducing GPU work for large multi-page documents.
    /// Leave as `None` until scroll infrastructure is implemented.
    visible_rect: Option<ViewportRect>,
) -> Element {
    // `visible_rect` is intentionally unused until the scroll pipeline lands.
    // Suppress the unused-variable warning explicitly so the seam stays visible.
    let _visible_rect = visible_rect;

    // `document_path` will be passed to the WGPU/Vello pipeline once real
    // document loading is implemented.
    let _document_path = document_path;

    // TODO: Acquire WGPU surface from Dioxus Native's windowing layer.
    //
    // When the Dioxus Native canvas API is stable, replace this div with:
    //   1. A native canvas element tied to a `wgpu::Surface`.
    //   2. A `vello::Renderer` created from the surface's device/queue.
    //   3. A `vello::Scene` populated via:
    //        loki_vello::paint_layout(&mut scene, &layout, &mut font_cache, (0.0, 0.0), scale);
    //   4. A render submission:
    //        renderer.render_to_surface(&device, &queue, &scene, &surface, &params)?;
    //
    // See `loki_vello::paint_layout` for the full rendering API.

    rsx! {
        // Canvas scroll container — flex: 1 lets it fill the space between
        // the two toolbars.  overflow: hidden prevents the page stub from
        // expanding the editor shell.
        div {
            style: format!(
                "flex: 1; overflow: hidden; background: {bg}; \
                 display: flex; justify-content: center; \
                 align-items: flex-start; padding: {pad}px;",
                bg  = theme::COLOR_SURFACE,
                pad = theme::SPACING_24,
            ),

            // A4 page stub — replace with a real WGPU canvas.
            // Dimensions: 794 × 1123 px (A4 at 96 dpi equivalent).
            div {
                style: format!(
                    "width: {w}px; height: {h}px; \
                     background: {bg}; flex-shrink: 0;",
                    w  = theme::PAGE_WIDTH_PX,
                    h  = theme::PAGE_HEIGHT_PX,
                    bg = theme::COLOR_PAGE_WHITE,
                ),
                // TODO: invoke loki_vello::render(scene, surface)
                //
                // Interface boundary:
                //   paint_layout(
                //       scene:      &mut vello::Scene,
                //       layout:     &loki_layout::DocumentLayout,
                //       font_cache: &mut loki_vello::FontDataCache,
                //       offset:     (f32, f32),   // document origin on canvas
                //       scale:      f32,           // HiDPI scale factor
                //   )
            }
        }
    }
}
