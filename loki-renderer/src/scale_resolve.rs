// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 AppThere Loki contributors

//! Resolving what the paint path actually renders at: the render mode, the zoom,
//! and the residency capability cap (Spec 08 T5.4).
//!
//! # These three are one cluster because their *order* is the correctness
//!
//! The mode has to be selected before the generation is read, the requested zoom
//! has to be recorded before the cap is applied, and the effective zoom has to be
//! read back after — because the CSS tile boxes are sized from it, and a box
//! sized from a zoom its texture was not rendered at paints the page at the wrong
//! size. And the editor's canonical layout has to be seeded after the mode and
//! before the cap, because the cap reads page sizes and the hand-over is a no-op
//! once anything has filled the cache. Written inline in a render body, that
//! ordering is five statements a later edit can reorder without any type
//! complaining — and one *was* reordered, silently costing a full second layout
//! per generation (see [`resolve`]). Here it is one function returning the one
//! value the caller needs, so there is nothing to reorder.
//!
//! Extracted from `document_view` when the capability call landed (r80) and took
//! the file over the 300-line ceiling.

use std::sync::Arc;

use appthere_canvas::residency::TextureBudget;

use crate::doc_page_source::DocPageSource;
use crate::render_layout::{reflow_layout_tile_width_pt, reflow_type_scale};
use crate::{RenderMode, ViewMode};

/// What the caller knows about this frame.
#[derive(Clone, Copy, PartialEq, Debug)]
pub(crate) struct ScaleInputs {
    pub view_mode: ViewMode,
    pub reflow_width_px: f64,
    /// The zoom the user requested, as a factor.
    pub zoom: f64,
    pub device_scale_factor: f64,
    pub texture_budget: TextureBudget,
}

/// Applies the mode, the requested zoom, the canonical layout and the capability
/// cap to `source`, and returns the zoom **actually rendered**.
///
/// The mode is applied rather than returned: the caller's only use for it was to
/// decide whether to seed the layout, which now happens here, and a returned
/// value with no reader is the shape that grows a second interpretation.
///
/// # `paginated_layout` is seeded here, and that placement is the fix to a
/// regression this function caused
///
/// The editor computes the document's layout once and hands it over
/// (`provide_paginated_layout`) so the renderer never lays it out a second time
/// — Tier-0 #3, the single canonical layout, and the reason the ~20 MB
/// system-font scan is skipped on open.
///
/// That hand-over is a **no-op once the cache holds the current generation**,
/// which makes it order-sensitive in a way nothing declared. When the capability
/// call landed (r80) it went in *ahead* of the seeding in `document_view`, and it
/// reads page sizes — so the renderer laid the document out itself, filled the
/// cache, and the editor's layout was then silently dropped. Every generation
/// paid a second full layout, and the first one on open paid the font scan too.
/// Nothing failed; the comment above the seeding call simply stopped being true.
///
/// So the seeding moves in here, between `set_render_mode` (which the layout must
/// be keyed against) and the capability call (which is the first reader). The
/// whole order is one function for the reason the module header gives: written as
/// statements in a render body, this is exactly the kind of edit that slips past.
pub(crate) fn resolve(
    source: &Arc<DocPageSource>,
    inputs: ScaleInputs,
    paginated_layout: Option<Arc<loki_layout::PaginatedLayout>>,
) -> f64 {
    // Select the render mode before reading the generation: switching mode
    // (or a reflow width change) invalidates the layout cache and advances
    // the generation so every tile repaints against the new layout.
    // Reflow runs the real layout engine at the viewport width (full
    // formatting fidelity), presented as zero-gap virtual tiles.
    let render_mode = if inputs.view_mode == ViewMode::Reflow && inputs.reflow_width_px > 1.0 {
        RenderMode::Reflow {
            available_width_pt: reflow_layout_tile_width_pt(inputs.reflow_width_px as f32),
        }
    } else {
        RenderMode::Paginated
    };
    source.set_render_mode(render_mode);
    // Paginated tiles zoom with the user's zoom control. Reflow tiles paint
    // at the responsive type scale (Spec 03 M4): the layout width above is
    // divided by the same factor, so the on-screen tile width is unchanged
    // while Compact type renders larger.
    let zoom = if render_mode == RenderMode::Paginated {
        // No floor here: `set_zoom` clamps to the shared residency range, and
        // a second literal 0.25 alongside `ZOOM_RANGE_MIN` is the duplication
        // that renaming those constants was meant to discourage.
        inputs.zoom
    } else {
        f64::from(reflow_type_scale(inputs.reflow_width_px as f32))
    };
    source.set_zoom(zoom as f32);

    // Seed the canonical layout **before anything reads one**. Reflow computes
    // its own width-dependent layout, so there is nothing to reuse there.
    if render_mode == RenderMode::Paginated
        && let Some(layout) = paginated_layout
    {
        source.provide_paginated_layout(layout);
    }

    // Spec 08 T5.4 requirement 1: apply the residency capability bound.
    //
    // Phase 2 derived this bound, tested it, and shipped it with no caller,
    // so `clamping_to_the_servable_zoom_makes_the_oom_branch_unreachable`
    // guarded a function nothing called. This is that caller, and it is a
    // *call* rather than a derivation on purpose — see `zoom_capability`.
    //
    // Placed after `set_zoom` and before the read-back so a single frame
    // never renders at a zoom the device cannot serve. Reflow mode is capped
    // too: its scale is the responsive type scale rather than the user's
    // zoom, but the residency cost is paid per texel either way.
    crate::zoom_capability::apply_to(source, inputs.device_scale_factor, inputs.texture_budget);

    // Read back rather than reuse the local. `set_zoom` records what was
    // *requested*; the source renders `min(requested, capability_limit)`, and
    // the CSS tile boxes below must be sized from what is actually rendered.
    // Sizing a box from a zoom the texture was not rendered at paints the
    // page at the wrong size — which the line above just made reachable.
    f64::from(source.zoom())
}

#[cfg(test)]
#[path = "scale_resolve_tests.rs"]
mod tests;
