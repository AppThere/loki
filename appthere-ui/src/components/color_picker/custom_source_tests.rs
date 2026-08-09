// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 AppThere Loki contributors

//! Which control wins, and what the handle does while the reader types.

use super::{displayed_hsv, fields_from_rgb, FieldMode, Source};

const DRAGGED: (f32, f32, f32) = (210.0, 60.0, 80.0);

fn close(a: f32, b: f32) -> bool {
    (a - b).abs() < 0.5
}

/// A dragged square owns the colour, whatever the fields happen to hold.
#[test]
fn the_dragged_square_wins_while_it_is_the_source() {
    let shown = displayed_hsv(Source::Area, DRAGGED, Some((0xFF, 0x00, 0x00)));
    assert_eq!(shown, DRAGGED);
}

/// A typed colour moves the handle — the direction that makes the square a
/// *view* of the fields rather than a second, competing store.
#[test]
fn a_typed_colour_moves_the_handle() {
    let shown = displayed_hsv(Source::Fields, DRAGGED, Some((0x00, 0xFF, 0x00)));
    assert!(
        close(shown.0, 120.0),
        "hue followed the field, got {shown:?}"
    );
    assert!(close(shown.1, 100.0));
    assert!(close(shown.2, 100.0));
}

/// **A half-typed field leaves the handle alone.** `#C0` is a real keystroke on
/// the way to `#C0392B`; a handle that jumped to the corner on every
/// unparseable intermediate would be unusable, and "unparseable" is the state a
/// field spends most of its typing life in.
#[test]
fn an_unparseable_field_leaves_the_handle_where_it_was() {
    assert_eq!(displayed_hsv(Source::Fields, DRAGGED, None), DRAGGED);
}

/// **A typed grey keeps the hue the reader chose.** Recomputing it would throw
/// the strip to red, and the reader would watch the strip move while typing a
/// value that has nothing to do with hue.
#[test]
fn a_typed_grey_keeps_the_dragged_hue() {
    let shown = displayed_hsv(Source::Fields, DRAGGED, Some((0x80, 0x80, 0x80)));
    assert!(close(shown.0, DRAGGED.0), "hue was replaced: {shown:?}");
    assert!(close(shown.1, 0.0), "a grey is unsaturated");
}

/// Black is the other colourless case — zero *value* rather than zero
/// saturation, and a separate branch in the conversion.
#[test]
fn typed_black_keeps_the_dragged_hue_too() {
    let shown = displayed_hsv(Source::Fields, DRAGGED, Some((0, 0, 0)));
    assert!(close(shown.0, DRAGGED.0), "{shown:?}");
    assert!(close(shown.2, 0.0));
}

/// **The polarity (L08-045).** Every test above passes for a function that
/// always returns `area_hsv`, which would make the fields unable to move the
/// handle at all — so pin that a real typed colour does override it.
#[test]
fn the_fields_are_not_simply_ignored() {
    let shown = displayed_hsv(Source::Fields, DRAGGED, Some((0x34, 0x98, 0xDB)));
    assert!(
        !close(shown.0, DRAGGED.0) || !close(shown.1, DRAGGED.1),
        "a typed colour must change something, got {shown:?}",
    );
}

/// **A dragged colour has a readable hex.** The first screen sitting showed a
/// filled preview swatch beside a blank `#` field: the reader had plainly chosen
/// a colour and had nowhere to read or copy it.
#[test]
fn a_dragged_colour_shows_its_value_in_the_fields() {
    let f = fields_from_rgb(FieldMode::Hex, (0xC0, 0x39, 0x2B));
    assert_eq!(f[0], "#C0392B");

    let f = fields_from_rgb(FieldMode::Rgb, (0xC0, 0x39, 0x2B));
    assert_eq!(
        [f[0].as_str(), f[1].as_str(), f[2].as_str()],
        ["192", "57", "43"]
    );
}

/// The displayed numbers are the colour's own, in each model — so a reader
/// switching models reads the same colour described differently rather than a
/// stale one.
#[test]
fn each_model_describes_the_same_colour() {
    let rgb = (0x00, 0xFF, 0x00);
    let hsv = fields_from_rgb(FieldMode::Hsv, rgb);
    assert_eq!(
        [hsv[0].as_str(), hsv[1].as_str(), hsv[2].as_str()],
        ["120", "100", "100"]
    );
    let hsl = fields_from_rgb(FieldMode::Hsl, rgb);
    assert_eq!(
        [hsl[0].as_str(), hsl[1].as_str(), hsl[2].as_str()],
        ["120", "100", "50"]
    );
}

/// **CMYK stays blank, on purpose.** This crate's CMYK conversion is the naive
/// complement, documented as adequate for a preview; deriving a display value
/// would give those numbers more authority than they have.
#[test]
fn cmyk_declines_to_state_a_value_it_cannot_support() {
    assert!(fields_from_rgb(FieldMode::Cmyk, (0xC0, 0x39, 0x2B))
        .iter()
        .all(String::is_empty));
}

/// Unused slots are empty, so a three-field model cannot show a fourth value
/// left over from a four-field one.
#[test]
fn unused_field_slots_are_empty() {
    assert!(fields_from_rgb(FieldMode::Hex, (1, 2, 3))[1..]
        .iter()
        .all(String::is_empty));
    assert!(fields_from_rgb(FieldMode::Rgb, (1, 2, 3))[3].is_empty());
}
