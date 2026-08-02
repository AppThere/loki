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
//! size. Written inline in a render body, that ordering is four statements a
//! later edit can reorder without any type complaining. Here it is one function
//! whose result is the pair the caller needs, so there is nothing to reorder.
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

/// Applies the mode, the requested zoom and the capability cap to `source`, and
/// returns the mode plus the zoom **actually rendered**.
pub(crate) fn resolve(source: &Arc<DocPageSource>, inputs: ScaleInputs) -> (RenderMode, f64) {
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
    let zoom = f64::from(source.zoom());

    (render_mode, zoom)
}
