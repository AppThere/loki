// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 AppThere Loki contributors

//! The stepping rules, the range, and the two conversions.

use super::{
    clamp_zoom_percent, next_zoom, parse_zoom_percent, prev_zoom, zoom_is_capped,
    zoom_percent_to_permille, ZOOM_MAX_PERCENT, ZOOM_MIN_PERCENT, ZOOM_PRESETS_PERCENT,
};

/// **The requirement, stated directly: the sequence never wraps.** This is the
/// test that fails against the old badge behaviour, where `next_zoom(200)` was
/// `50` — a zoom-**in** press that lands further out than it started.
#[test]
fn stepping_saturates_and_never_wraps() {
    assert_eq!(next_zoom(ZOOM_MAX_PERCENT), ZOOM_MAX_PERCENT);
    assert_eq!(
        next_zoom(1000),
        ZOOM_MAX_PERCENT,
        "above the range, still in"
    );
    assert_eq!(prev_zoom(ZOOM_MIN_PERCENT), ZOOM_MIN_PERCENT);
    assert_eq!(prev_zoom(1), ZOOM_MIN_PERCENT, "below the range, still out");
}

/// Stepping is monotonic in the direction it names, at every preset — the
/// property that makes "never wraps" mean something at the interior points too,
/// not only at the ends.
#[test]
fn every_step_moves_in_its_own_direction() {
    for p in ZOOM_PRESETS_PERCENT {
        assert!(next_zoom(p) >= p, "next_zoom({p}) went down");
        assert!(prev_zoom(p) <= p, "prev_zoom({p}) went up");
    }
}

/// In and out are inverses in the interior of the ladder. Checked as a round
/// trip rather than a table so it keeps holding when a preset is added — and it
/// is deliberately not asserted at the ends, where saturation makes them not
/// inverses and that is the intended behaviour.
#[test]
fn in_then_out_returns_to_the_same_preset() {
    for p in ZOOM_PRESETS_PERCENT {
        if p == ZOOM_MAX_PERCENT {
            continue;
        }
        assert_eq!(prev_zoom(next_zoom(p)), p, "round trip at {p}%");
    }
}

/// **A zoom between presets rejoins the ladder rather than jumping to a fixed
/// rung.** This is what a typed 137% or a Fit Width result does, and a stepper
/// written as a `match` on exact values — which this replaced — sent all of them
/// to 100%.
#[test]
fn a_value_between_presets_steps_to_its_neighbours() {
    assert_eq!(next_zoom(137), 150);
    assert_eq!(prev_zoom(137), 125);
    assert_eq!(next_zoom(101), 125);
    assert_eq!(prev_zoom(99), 75);
}

/// The presets are ascending and inside the range. An out-of-order or
/// out-of-range entry breaks `find`/`rev().find()` silently — stepping would
/// still return *a* number.
#[test]
fn the_presets_are_ordered_and_in_range() {
    for pair in ZOOM_PRESETS_PERCENT.windows(2) {
        assert!(pair[0] < pair[1], "presets not ascending at {pair:?}");
    }
    for p in ZOOM_PRESETS_PERCENT {
        assert!((ZOOM_MIN_PERCENT..=ZOOM_MAX_PERCENT).contains(&p), "{p}%");
    }
}

/// Typing accepts what a person actually types, and refuses what is not a
/// number — `None` rather than a default, so a stray keystroke leaves the field
/// where it was instead of snapping the document to 100%.
#[test]
fn typed_zoom_is_forgiving_about_form_and_strict_about_content() {
    assert_eq!(parse_zoom_percent("150"), Some(150));
    assert_eq!(parse_zoom_percent(" 150% "), Some(150));
    assert_eq!(parse_zoom_percent("150 %"), Some(150));
    assert_eq!(parse_zoom_percent(""), None);
    assert_eq!(parse_zoom_percent("abc"), None);
    assert_eq!(parse_zoom_percent("1e3"), None);
}

/// **A typed value out of range is clamped, not rejected.** Rejecting would
/// leave the user's "900" in the field with nothing happening, which reads as a
/// broken control; clamping shows them the bound.
#[test]
fn a_typed_value_outside_the_range_lands_on_the_bound() {
    assert_eq!(parse_zoom_percent("900"), Some(ZOOM_MAX_PERCENT));
    assert_eq!(parse_zoom_percent("1"), Some(ZOOM_MIN_PERCENT));
    assert_eq!(clamp_zoom_percent(0), ZOOM_MIN_PERCENT);
}

/// Percent → permille is exact at every preset. The whole reason requirement 2
/// exists is a bound comparison that missed by 4e-15; integer maths cannot, and
/// this is the assertion that a future "just use f32 here" would fail.
#[test]
fn the_permille_conversion_is_exact() {
    assert_eq!(zoom_percent_to_permille(100), 1000);
    assert_eq!(zoom_percent_to_permille(600), 6000);
    for p in ZOOM_PRESETS_PERCENT {
        assert_eq!(u32::from(zoom_percent_to_permille(p)), p * 10);
    }
}

/// The indicator appears exactly when something was taken away.
#[test]
fn the_capped_indicator_tracks_a_real_reduction() {
    assert!(
        zoom_is_capped(300, Some(1500)),
        "3000 permille held to 1500"
    );
    assert!(
        !zoom_is_capped(100, Some(1500)),
        "the cap is above the request"
    );
}

/// **Both polarities of the boundary (L08-045).** A limit *equal* to the request
/// took nothing away, so the indicator must not fire — and with no limit at all
/// there is nothing to report. Either mistake gives the reader a warning about a
/// reduction that did not happen, which is how an indicator becomes furniture.
#[test]
fn an_equal_or_absent_limit_is_not_a_reduction() {
    assert!(
        !zoom_is_capped(150, Some(1500)),
        "a limit equal to the request is not a reduction",
    );
    assert!(!zoom_is_capped(600, None));
    assert!(
        zoom_is_capped(150, Some(1499)),
        "one permille below the request is",
    );
}
