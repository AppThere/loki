// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 AppThere Loki contributors

//! Ctrl+wheel zoom on the document canvas (Spec 08 T5.6).
//!
//! # The gesture had to be built before it could be handled
//!
//! Pointer-anchored zoom was specified in T5.6 and `ZoomAnchor::Viewport` was
//! written for it, but nothing produced one: Blitz consumed `WindowEvent::
//! MouseWheel` entirely inside the shell, so no wheel gesture reached Dioxus and
//! `onwheel` was `unimplemented!()` in `dioxus-native-dom`. The register carried
//! that as an open row — a decision with no consumer — and this is the consumer.
//! The chain is three `PATCH(loki)` edits: `Document::handle_wheel` in blitz-dom,
//! the report-then-decline in blitz-shell's `MouseWheel` arm, and the `wheel`
//! dispatch in `dioxus-native-dom`. See `docs/patches.md`.
//!
//! # The shell declines the scroll; this does not have to
//!
//! Ctrl+wheel must zoom *without* also scrolling, and only the shell can arrange
//! that — by the time a handler here runs, a scroll would already have happened.
//! So the shell declines to scroll any wheel carrying a modifier. Deliberately
//! *any*: the set below is the app's choice and the shell cannot know it, and two
//! modifier lists that have to agree across a crate boundary is exactly how a
//! wheel comes to zoom and scroll at once. The shell's rule is the broader one,
//! so it cannot be narrower than this, whatever this becomes.
//!
//! The modifier test here is therefore not a second gate on the same fact: it is
//! the reason this handler does anything at all, since **every** wheel is
//! reported, including the ones that scroll.
//!
//! # Multiplicative, because zoom is
//!
//! A fixed ±10 percentage points is a 40% jump at 25% zoom and a 1.7% nudge at
//! 600%. A fixed *factor* is the same visual step everywhere, which is what makes
//! a wheel feel like a zoom rather than like a slider with a badly chosen scale.
//!
//! The preset ladder (`next_zoom`/`prev_zoom`) is deliberately not used: it has
//! nine rungs across 25–600 and a trackpad delivers tens of events per second, so
//! one flick would cross the whole range. The ladder is for the buttons, where a
//! press is a discrete intent.

use appthere_ui::scroll::ZoomAnchor;
use appthere_ui::{ZOOM_MAX_PERCENT, ZOOM_MIN_PERCENT, clamp_zoom_percent};
use dioxus::html::geometry::WheelDelta;
use dioxus::native::NativeWheelData;
use dioxus::prelude::*;
use keyboard_types::Modifiers;

use super::editor_zoom::ZoomCommand;

/// Zoom factor applied per line of wheel movement.
///
/// 1.1 gives roughly seven notches per doubling, which on a notched wheel is a
/// deliberate-feeling step and on a trackpad is smooth once the per-line scale
/// below converts to it.
const FACTOR_PER_LINE: f32 = 1.1;

/// Pixels of wheel movement treated as one line.
///
/// A guess, and unavoidably one: the platform reports pixels for a trackpad and
/// lines for a wheel, and nothing in this path knows a line height. It is used
/// **only** for gesture feel — never to convert a reported delta into a claim
/// about distance — which is why it lives here in the app's policy rather than
/// in the patched event chain, where it would have looked like a measurement.
const PIXELS_PER_LINE: f32 = 50.0;

/// A wheel delta as a signed count of lines, sign following the platform.
///
/// Positive means the wheel was pushed away from the reader, which is zoom *in*
/// on every desktop platform. This is winit's sign convention, passed through
/// the patched chain unchanged; note it is the **opposite** of the web's
/// `deltaY`, where positive means scrolling down.
///
/// `Pages` is folded to one line per page rather than ignored: a platform that
/// reports pages is rare, and a gesture that does nothing is worse than one that
/// moves conservatively.
#[must_use]
fn lines_from(delta: WheelDelta) -> f32 {
    match delta {
        WheelDelta::Pixels(v) => v.y as f32 / PIXELS_PER_LINE,
        WheelDelta::Lines(v) => v.y as f32,
        WheelDelta::Pages(v) => v.y as f32,
    }
}

/// The zoom a wheel gesture of `delta` moves `current` to.
///
/// # It never stalls
///
/// The zoom is a whole percent, so a small trackpad delta can round back to the
/// value it started from — and at the bottom of the range it always does: 1% of
/// 20 is 0.2. Without the floor below, the control would work near 100% and be
/// dead at 25%, which reads as the gesture being unsupported rather than as a
/// rounding bug.
///
/// So a non-zero gesture moves at least one percent, in its own direction. The
/// clamp still holds: at the range ends the result is the end, and the step is
/// the one place that could otherwise walk past it.
#[must_use]
pub(super) fn wheel_zoom_percent(current: u32, delta: WheelDelta) -> u32 {
    let lines = lines_from(delta);
    if lines == 0.0 || !lines.is_finite() {
        return current;
    }
    let factor = FACTOR_PER_LINE.powf(lines);
    let stepped = (current as f32 * factor).round() as i64;
    let stepped = if stepped == i64::from(current) {
        // Rounded back to where it started — move one percent instead.
        i64::from(current) + if lines > 0.0 { 1 } else { -1 }
    } else {
        stepped
    };
    let bounded = stepped.clamp(i64::from(ZOOM_MIN_PERCENT), i64::from(ZOOM_MAX_PERCENT));
    // `clamp_zoom_percent` is the range's single owner; the clamp above only
    // brings the value into `u32` so this one can be the one that decides.
    clamp_zoom_percent(bounded as u32)
}

/// The canvas `onwheel` handler: Ctrl+wheel zooms about the pointer.
///
/// Returns a closure so the canvas — a plain function with no hook scope — can
/// mount it without owning any of this.
pub(super) fn make_wheel_handler(
    zoom: ZoomCommand,
) -> impl FnMut(dioxus::events::WheelEvent) + 'static {
    move |evt: dioxus::events::WheelEvent| {
        // `modifiers()` carries the live modifier state the shell read at the
        // moment of the gesture, not a remembered one.
        //
        // The same three the keyboard path checks: Ctrl on Windows/Linux, and
        // macOS Cmd, which blitz-shell maps to SUPER rather than META — so both
        // are tested, as `editor_keydown` does.
        let mods = evt.modifiers();
        if !(mods.ctrl() || mods.meta() || mods.contains(Modifiers::SUPER)) {
            return; // an ordinary scroll — the shell already handled it
        }
        // The anchor frame, before the zoom: `ZoomAnchor::Viewport` is defined
        // in the scrollport's coordinates, and `element_coordinates()` is
        // **not** in them — it is relative to the event's target, which for a
        // wheel over text is the text run, at whatever position it occupies
        // inside the scrolled content. The first sitting measured the gap
        // directly: pointer at window y 194, `element_y` 329, because the page
        // element's top was 135 px above the window. So the anchor comes from
        // the scrollport frame the patched event carries for exactly this.
        //
        // No scrollport means nothing here scrolls, so there is nothing for an
        // anchor to hold still — and `Centre` would be a different answer to a
        // question that has none.
        let payload = evt.data();
        let Some(data) = payload.downcast::<NativeWheelData>() else {
            return;
        };
        let Some((ax, ay)) = data.scrollport else {
            return;
        };
        let current = zoom.percent();
        let next = wheel_zoom_percent(current, data.delta);
        if next == current {
            return;
        }
        zoom.set(
            next,
            ZoomAnchor::Viewport {
                x: ax as f32,
                y: ay as f32,
            },
        );
    }
}

#[cfg(test)]
#[path = "editor_wheel_zoom_tests.rs"]
mod tests;
