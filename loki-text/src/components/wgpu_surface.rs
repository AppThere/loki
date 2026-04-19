// Copyright 2026 AppThere Loki contributors
// SPDX-License-Identifier: Apache-2.0

//! WGPU document canvas component.
//!
//! [`WgpuSurface`] is the integration point between the Dioxus Native UI tree
//! and the [`loki_vello`] GPU rendering pipeline.  For this session the surface
//! builds a [`vello::Scene`] containing a blank white A4 page rectangle (white
//! fill + 1 px border), establishing the Vello render loop.  Scene submission
//! to a real GPU surface is stubbed pending Dioxus Native canvas API stability.
//!
//! # Integration seam
//!
//! When Dioxus Native exposes stable window-handle access, replace the
//! `_scene` binding with:
//!
//! 1. Obtain a `wgpu::Surface` from the window handle.
//! 2. Create (or reuse across renders) a `vello::Renderer`.
//! 3. Submit via `renderer.render_to_surface(&device, &queue, &scene, &surface, &params)`.
//!
//! `loki_vello::paint_layout` can then populate the scene with real document
//! content once a [`loki_layout::DocumentLayout`] is available.

use dioxus::prelude::*;
use kurbo::{Affine, Rect, Stroke};
use loki_theme::tokens;
use peniko::{Brush, Color, Fill};
use vello::Scene;

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
/// Occupies the `flex: 1` region between the top and bottom toolbars.  Each
/// render call builds a [`vello::Scene`] containing a blank A4 page (white fill
/// + 1 px border) via [`kurbo`] geometry primitives.  The scene is not yet
/// submitted to a GPU surface — see the module-level doc for the pending steps.
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
    // Build the Vello scene for this render cycle.
    let _scene = build_page_scene(document_path.as_deref(), visible_rect.as_ref());

    // TODO: When Dioxus Native canvas API is stable, obtain a wgpu::Surface
    // from the window handle and submit `_scene`:
    //     renderer.render_to_surface(&device, &queue, &_scene, &surface, &params)?;

    rsx! {
        // Canvas scroll container — flex: 1 fills the space between toolbars.
        div {
            style: format!(
                "flex: 1; overflow: hidden; background: {bg}; \
                 display: flex; justify-content: center; \
                 align-items: flex-start; padding: {pad}px;",
                bg  = tokens::COLOR_SURFACE_BASE,
                pad = tokens::SPACE_6,
            ),

            // A4 page placeholder — rendered as a div until the wgpu canvas
            // API is available to display the Vello scene directly.
            div {
                style: format!(
                    "width: {w}px; height: {h}px; \
                     background: {bg}; flex-shrink: 0; \
                     border: 1px solid {border};",
                    w      = tokens::PAGE_WIDTH_PX,
                    h      = tokens::PAGE_HEIGHT_PX,
                    bg     = tokens::COLOR_SURFACE_PAGE,
                    border = tokens::COLOR_BORDER_DEFAULT,
                ),
            }
        }
    }
}

// ── Scene construction ────────────────────────────────────────────────────────

/// Build a [`vello::Scene`] containing a blank A4 page.
///
/// Draws a white-filled rectangle at A4 dimensions with a 1 px border.
/// When document content is available, `loki_vello::paint_layout` should be
/// called after this to layer real content on top.
fn build_page_scene(_path: Option<&str>, visible_rect: Option<&ViewportRect>) -> Scene {
    let mut scene = Scene::new();

    // TODO(partial-render): replace with viewport-clipped scene when visible_rect is Some.
    let _ = visible_rect;

    let page = Rect::new(
        0.0,
        0.0,
        tokens::PAGE_WIDTH_PX as f64,
        tokens::PAGE_HEIGHT_PX as f64,
    );

    // White page fill.
    let white = Brush::Solid(Color::new([1.0_f32, 1.0, 1.0, 1.0]));
    scene.fill(Fill::NonZero, Affine::IDENTITY, &white, None, &page);

    // 1 px border (#E0E0E0 — COLOR_BORDER equivalent as linear f32).
    let border = Brush::Solid(Color::new([0.878_f32, 0.878, 0.878, 1.0]));
    scene.stroke(&Stroke::new(1.0), Affine::IDENTITY, &border, None, &page);

    scene
}
