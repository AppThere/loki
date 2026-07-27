// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 AppThere Loki contributors

//! The placement algorithm itself. Split from `geometry.rs` so the shapes and
//! the arithmetic stay separately readable and both stay under the ceiling.

use super::{Align, Placement, PlacementRequest, Rect, Side};

/// Places an overlay against its anchor, inside its viewport.
///
/// # Order of concessions
///
/// 1. **Preferred side at full size**, if it fits.
/// 2. **Flip** to the other side, if the overlay fits fully there.
/// 3. **Clamp height** to whichever side has more room. The overlay is expected
///    to scroll; [`Placement::clamped`] says so.
/// 4. **Shift** across the axis so the overlay stays inside the viewport, even
///    where that breaks the requested alignment.
///
/// Flip before clamp is the important ordering: a fully-visible list on the
/// other side beats a truncated one on the preferred side, and the reverse
/// order would truncate a menu that had somewhere to go.
///
/// # Never covers its own anchor
///
/// With a non-negative `gap` the placed rect and the anchor cannot overlap on
/// either side, which is asserted rather than argued. A flip computed with the
/// wrong sign puts the overlay *over* the control that opened it — a failure
/// that reads as "the menu is in roughly the right place" until someone tries to
/// use it.
#[must_use]
pub fn place(req: PlacementRequest) -> Placement {
    let vp = req.viewport;
    let margin = req.margin.max(0.0);
    let gap = req.gap.max(0.0);

    // Usable space, after the edge margins. A viewport smaller than its own
    // margins yields zero rather than a negative, so the arithmetic below stays
    // meaningful instead of producing an inverted rect.
    let usable_w = (vp.width - 2.0 * margin).max(0.0);
    let width = req.width.min(usable_w).max(0.0);

    let room_above = (req.anchor.y - vp.y - gap - margin).max(0.0);
    let room_below = (vp.bottom() - req.anchor.bottom() - gap - margin).max(0.0);
    let wanted = req.height.max(0.0);

    // Side selection, in one comparison rather than three branches. An earlier
    // draft tested "does the preferred side fit / does the other fit / which is
    // roomier" separately, and a mutation removing the middle test changed
    // nothing: when the preferred side does not fit, `other_room >= wanted`
    // already implies `other_room > preferred_room`, so that branch was
    // unreachable-by-subsumption. The mutation surviving was the finding.
    //
    // Flip-before-clamp is therefore not a separate rule to get right — it falls
    // out. Take the preferred side when it fits or when it is the roomier of the
    // two; otherwise flip. Height is clamped afterwards to whatever the chosen
    // side actually has.
    //
    // # PRECONDITION: `wanted` is the same on both sides
    //
    // The subsumption above depends on it. `preferred_room < wanted` and
    // `other_room >= wanted` give `other_room > preferred_room` **only because
    // `wanted` is one quantity**. Make the requirement side-dependent — a caret
    // that draws only when the popover opens downward, asymmetric padding, a
    // shadow on one side — and a side can *fit* while being *less roomy* than
    // the preferred side. The collapsed comparison would then pick the roomier
    // side and clip, and it would clip only in one direction, which is the
    // expensive kind of bug to chase.
    //
    // A caret is the most likely thing to be added to a popover once it works,
    // so this is a near-future hazard rather than a theoretical one. If the
    // requirement ever becomes side-dependent, restore the explicit
    // does-it-fit test for each side — and
    // `a_request_that_fits_on_either_side_is_never_clamped` will fail until you
    // do.
    let preferred_room = room_for(req.preferred, room_above, room_below);
    let other = req.preferred.flipped();
    let other_room = room_for(other, room_above, room_below);
    let (side, room) = if preferred_room >= wanted || preferred_room >= other_room {
        (req.preferred, preferred_room)
    } else {
        (other, other_room)
    };

    let height = wanted.min(room);
    let unclamped_y = match side {
        Side::Below => req.anchor.bottom() + gap,
        Side::Above => req.anchor.y - gap - height,
    };
    // Clamp on the main axis too, not only across it.
    //
    // Every other assertion in this module is "the overlay is inside the
    // viewport", and without this that property was **conditional on the anchor
    // being inside the viewport** — an unstated precondition, which is the
    // surface-that-permits failure L08-043 is about. It surfaced from a real
    // fixture: a click 2px from the top of an Android window whose safe area
    // starts at 34px put the menu at y=24, under the status bar.
    //
    // A caller whose anchor has left the viewport gets an overlay at the edge
    // rather than off-screen, and `interaction::anchor_is_anchorable` — the one
    // place the boundary is decided — makes `on_anchor_change` dismiss it on the
    // next frame. So the clamp is a floor under a one-frame transient, not a new
    // behaviour anyone will rely on. That sentence used to be an assumption
    // about a caller's `bool`; it is now a property of a shared predicate, and
    // `the_two_modules_agree_on_the_anchor_visibility_boundary` holds it.
    let y_lo = vp.y + margin;
    let y_hi = (vp.bottom() - margin - height).max(y_lo);
    let y = unclamped_y.clamp(y_lo, y_hi);

    let ideal_x = match req.align {
        Align::Start => req.anchor.x,
        Align::Center => req.anchor.x + req.anchor.width / 2.0 - width / 2.0,
        Align::End => req.anchor.right() - width,
    };
    let lo = vp.x + margin;
    let hi = (vp.right() - margin - width).max(lo);
    let x = ideal_x.clamp(lo, hi);

    Placement {
        rect: Rect::new(x, y, width, height),
        side,
        flipped: side != req.preferred,
        // Compared against the ideal rather than recomputed from the result: the
        // question is "did alignment survive", and only the ideal knows what
        // alignment asked for.
        shifted: (x - ideal_x).abs() > f32::EPSILON || (y - unclamped_y).abs() > f32::EPSILON,
        clamped: height < wanted || width < req.width,
    }
}

fn room_for(side: Side, above: f32, below: f32) -> f32 {
    match side {
        Side::Above => above,
        Side::Below => below,
    }
}
