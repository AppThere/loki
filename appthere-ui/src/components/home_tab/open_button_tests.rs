// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 AppThere Loki contributors

//! T4.3's one decision — tooltip or visible label — and the tooltip's geometry.
//!
//! **This is the branch T4.0 recorded as never having run.** `note_pointer` had
//! no production caller and `has_hover` no production reader, so
//! `DeviceProfile::pointer` was permanently `Unknown` and only one arm existed in
//! practice. Both arms are asserted here, which is what makes the screen session
//! a confirmation rather than the first execution.

use super::{shows_visible_label, tooltip_placement};
use crate::components::popover::{place, Rect, Side, MIN_ANCHORED_HEIGHT_PX};
use crate::device_profile::PointerPrecision;

const VIEWPORT: Rect = Rect {
    x: 0.0,
    y: 0.0,
    width: 1280.0,
    height: 800.0,
};

/// The Open button at the top of the Recent section.
fn button_at(y: f32) -> Rect {
    Rect {
        x: 40.0,
        y,
        width: 44.0,
        height: 44.0,
    }
}

/// **A coarse pointer cannot reach a tooltip, so the name must be visible.**
/// An icon button whose only label is a hover affordance is, on touch, a control
/// the user has to guess.
#[test]
fn a_coarse_pointer_gets_the_visible_label() {
    assert!(shows_visible_label(PointerPrecision::Coarse));
}

/// **The default is the label, and that direction is the decision.** `Unknown`
/// means nobody has pointed at the app yet — a touch-only session's first frame.
/// Guessing "hover works" there would ship an unnamed icon to exactly the users
/// who cannot discover it; guessing the other way costs a mouse user a visible
/// word until their first hover.
#[test]
fn an_unprobed_pointer_gets_the_visible_label_too() {
    assert!(
        shows_visible_label(PointerPrecision::Unknown),
        "until we know, the affordance must be visible — an unreachable tooltip \
         is a worse failure than a redundant label",
    );
}

/// **The polarity that makes the two above mean something (L08-045):** with a
/// mouse the tooltip form is used. Without this, `shows_visible_label` returning
/// `true` unconditionally passes both, and T4.3 would have shipped one arm.
#[test]
fn a_fine_pointer_gets_the_tooltip() {
    assert!(!shows_visible_label(PointerPrecision::Fine));
}

/// A device with **both** — an Android desktop with a touchscreen and a mouse —
/// has hover available, so the tooltip is reachable. `Both` is not a tie to be
/// broken; it means every affordance works.
#[test]
fn a_device_with_both_pointers_gets_the_tooltip() {
    assert!(!shows_visible_label(PointerPrecision::Both));
}

/// The tooltip opens below its button and stays on screen. Below because this
/// button sits at the top of its section, where above is the window edge.
#[test]
fn the_tooltip_opens_below_the_button() {
    let anchor = button_at(120.0);
    let req = tooltip_placement(anchor);
    let p = place(req);
    assert_eq!(p.side, Side::Below);
    assert!(
        (p.rect.y - (anchor.bottom() + req.gap)).abs() < 0.001,
        "tooltip top {} is not the button's bottom plus the {}px gap",
        p.rect.y,
        req.gap,
    );
}

/// It must not cover the control it describes — a tooltip over its own button is
/// the affordance eating itself, and the pointer is by definition on that button.
#[test]
fn the_tooltip_never_covers_its_own_button() {
    for step in 0..=40 {
        let anchor = button_at(step as f32 * 20.0);
        let mut req = tooltip_placement(anchor);
        req.viewport = VIEWPORT;
        let p = place(req);
        assert!(
            !p.rect.covers_vertically_open(anchor),
            "tooltip {:?} covers its button {:?}",
            p.rect,
            anchor,
        );
        assert!(p.rect.is_inside(VIEWPORT), "{:?}", p.rect);
    }
}

/// One line is the whole tooltip, so its floor is one touch target rather than
/// the two-row menu minimum: there is no "the list continues" to conceal, which
/// is the only thing the larger minimum protects.
#[test]
fn the_tooltip_floor_is_one_touch_target_not_two_rows() {
    assert!(
        (tooltip_placement(button_at(120.0)).min_anchored_height - MIN_ANCHORED_HEIGHT_PX).abs()
            < f32::EPSILON,
    );
}
