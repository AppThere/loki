// Copyright 2026 AppThere Loki contributors
// SPDX-License-Identifier: Apache-2.0

//! WGPU document canvas component.
//!
//! [`WgpuSurface`] is the integration point between the Dioxus Native UI tree
//! and the [`loki_vello`] GPU rendering pipeline.  When a
//! [`loki_doc_model::Document`] is available the surface builds a real Vello
//! scene via `loki-layout` → `loki-vello`.  While no document is loaded (or
//! while the document is still loading) a blank white A4 placeholder scene is
//! produced instead.
//!
//! Scene submission to a live GPU surface is blocked pending Dioxus Native
//! canvas API stability — see the BLOCKED comment in [`WgpuSurface`].
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
//! `loki_vello::paint_layout` already populates the scene with real document
//! content when a [`loki_doc_model::Document`] is present.

use std::cell::RefCell;
use std::rc::Rc;

use dioxus::prelude::*;
use kurbo::{Affine, Rect, Stroke};
use loki_doc_model::document::Document;
use loki_layout::{layout_document, FontResources, LayoutMode};
use loki_theme::tokens;
use loki_vello::{paint_layout, FontDataCache};
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

// ── WgpuSurface props ─────────────────────────────────────────────────────────

/// Props for [`WgpuSurface`].
///
/// [`Document`] does not implement [`PartialEq`], so the props struct provides
/// a conservative `PartialEq` (always `false`) ensuring re-renders are never
/// incorrectly skipped.
#[derive(Clone, Props)]
pub struct WgpuSurfaceProps {
    /// Document to render.  `None` shows a blank A4 placeholder (used during
    /// loading and when no file is open).
    pub document: Option<Document>,

    /// Currently visible portion of the document canvas.
    ///
    /// # Future work
    ///
    /// Populate with the current scroll viewport in document-space coordinates.
    /// The renderer should cull items outside this rect before building the
    /// Vello scene, reducing GPU work for large multi-page documents.
    /// Leave as `None` until scroll infrastructure is implemented.
    pub visible_rect: Option<ViewportRect>,
}

// Document does not implement PartialEq; conservatively always re-render.
impl PartialEq for WgpuSurfaceProps {
    fn eq(&self, _: &Self) -> bool {
        false
    }
}

// ── WgpuSurface ───────────────────────────────────────────────────────────────

/// WGPU document canvas component.
///
/// Occupies the `flex: 1` region between the top and bottom toolbars.
///
/// When `document` is `Some`, the component:
/// 1. Runs `loki-layout` to produce a [`loki_layout::DocumentLayout`].
/// 2. Calls [`loki_vello::paint_layout`] to translate layout items into a
///    Vello scene.
///
/// When `document` is `None`, a blank white A4 page placeholder scene is used.
///
/// In both cases, scene submission to a real GPU surface is **blocked** —
/// see the BLOCKED comment in the function body.
#[allow(non_snake_case)]
pub fn WgpuSurface(props: WgpuSurfaceProps) -> Element {
    let WgpuSurfaceProps { document, visible_rect } = props;

    // FontResources is expensive to create (system font discovery via fontique).
    // Cache it across renders with Rc<RefCell<>> since FontResources: !Clone.
    let font_resources = use_hook(|| Rc::new(RefCell::new(FontResources::new())));

    // Build the Vello scene for this render cycle.
    //
    // BLOCKED(canvas-api): Dioxus Native 0.7 does not yet expose a stable
    // wgpu::Surface from the window handle. The scene is constructed but not
    // submitted to the GPU. Replace `_scene` with the submission sequence once
    // the canvas API stabilises:
    //     renderer.render_to_surface(&device, &queue, &_scene, &surface, &params)?;
    // Adapted from loki-vello/examples/render_to_png.rs — submission point.
    let _scene = build_page_scene(
        document.as_ref(),
        visible_rect.as_ref(),
        &mut *font_resources.borrow_mut(),
    );

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

/// Build a [`vello::Scene`] for the current document state.
///
/// When `document` is `Some`, runs the full layout + paint pipeline:
/// [`layout_document`] → [`paint_layout`].
///
/// When `document` is `None`, falls back to a blank A4 placeholder (white fill
/// + 1 px border).
///
/// `font_resources` is borrowed mutably so font-discovery state is reused
/// across calls without re-scanning system fonts each render.
fn build_page_scene(
    document: Option<&Document>,
    visible_rect: Option<&ViewportRect>,
    font_resources: &mut FontResources,
) -> Scene {
    let mut scene = Scene::new();

    // TODO(partial-render): replace with viewport-clipped scene when visible_rect is Some.
    let _ = visible_rect;

    if let Some(doc) = document {
        // Full pipeline: Document → DocumentLayout → vello::Scene.
        // FontDataCache is cheap to create (empty HashMap); a fresh instance
        // per call is acceptable while GPU submission is blocked.
        let layout = layout_document(font_resources, doc, LayoutMode::Pageless, 1.0);
        let mut font_cache = FontDataCache::new();
        paint_layout(&mut scene, &layout, &mut font_cache, (0.0, 0.0), 1.0);
        return scene;
    }

    // Placeholder: blank white A4 page with a 1 px border.
    let page = Rect::new(
        0.0,
        0.0,
        tokens::PAGE_WIDTH_PX as f64,
        tokens::PAGE_HEIGHT_PX as f64,
    );

    let white = Brush::Solid(Color::new([1.0_f32, 1.0, 1.0, 1.0]));
    scene.fill(Fill::NonZero, Affine::IDENTITY, &white, None, &page);

    // 1 px border (#E0E0E0 — COLOR_BORDER equivalent as linear f32).
    let border = Brush::Solid(Color::new([0.878_f32, 0.878, 0.878, 1.0]));
    scene.stroke(&Stroke::new(1.0), Affine::IDENTITY, &border, None, &page);

    scene
}
