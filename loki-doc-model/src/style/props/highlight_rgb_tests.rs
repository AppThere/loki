// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 AppThere Loki contributors

//! The named-highlight palette, and the reverse lookup that keeps a typed hex
//! and a clicked swatch on the same route.

use super::{HIGHLIGHT_RGB, HighlightColor};

/// **Every named variant is in the table.** `HighlightColor` is `#[non_exhaustive]`
/// and this list is written by hand, so a variant added later would otherwise
/// return `None` from `to_hex` and route to a custom colour — the same yellow
/// exporting two ways, which is the failure this module exists to prevent.
///
/// Written as an explicit list rather than a count so the failure names the
/// missing colour instead of a number.
#[test]
fn every_named_highlight_has_a_colour() {
    for v in [
        HighlightColor::Yellow,
        HighlightColor::Green,
        HighlightColor::Cyan,
        HighlightColor::Magenta,
        HighlightColor::Blue,
        HighlightColor::Red,
        HighlightColor::DarkBlue,
        HighlightColor::DarkCyan,
        HighlightColor::DarkGreen,
        HighlightColor::DarkMagenta,
        HighlightColor::DarkRed,
        HighlightColor::DarkYellow,
        HighlightColor::DarkGray,
        HighlightColor::LightGray,
        HighlightColor::Black,
        HighlightColor::White,
    ] {
        assert!(v.to_hex().is_some(), "{v:?} has no colour");
    }
}

/// **And `None` has none.** It means *no highlight*; a colour for it would be a
/// colour to paint, and `to_rgb` returning something for it would make an
/// explicit "remove the highlight" paint grey.
#[test]
fn none_is_not_a_colour() {
    assert_eq!(HighlightColor::None.to_hex(), None);
    assert_eq!(HighlightColor::None.to_rgb(), None);
}

/// Round trip: every colour in the table resolves back to its own variant.
#[test]
fn each_colour_resolves_to_its_own_variant() {
    for (variant, hex) in HIGHLIGHT_RGB {
        assert_eq!(HighlightColor::from_hex(hex), Some(*variant), "{hex}");
    }
}

/// **No two variants share a colour**, which is what makes the round trip above
/// a round trip rather than a lookup that happens to agree.
#[test]
fn the_colours_are_distinct() {
    for (i, (_, a)) in HIGHLIGHT_RGB.iter().enumerate() {
        for (_, b) in &HIGHLIGHT_RGB[i + 1..] {
            assert_ne!(a, b, "two variants share {a}");
        }
    }
}

/// **A colour off the palette is custom, not snapped.** One unit away from
/// yellow is a colour the reader chose; nearest-match would discard it silently,
/// and the model can now carry it.
#[test]
fn a_near_miss_is_not_a_named_highlight() {
    assert_eq!(HighlightColor::from_hex("#FFFF01"), None);
    assert_eq!(HighlightColor::from_hex("#FEFF00"), None);
    assert_eq!(HighlightColor::from_hex("#C0392B"), None);
}

/// The lookup does not care how the hex was written — a typed `ffff00` and a
/// swatch's `#FFFF00` are the same colour, and routing them differently is the
/// defect this whole module is about.
#[test]
fn case_and_hash_do_not_change_the_answer() {
    for s in ["#FFFF00", "#ffff00", "ffff00", " #FfFf00 "] {
        assert_eq!(
            HighlightColor::from_hex(s),
            Some(HighlightColor::Yellow),
            "{s}"
        );
    }
}

/// Malformed input is `None` rather than a panic or a wrong colour — the hex
/// field can hold anything mid-typing.
#[test]
fn malformed_input_is_refused() {
    for s in ["", "#", "#FFF", "#FFFF0", "#FFFF000", "#GGGG00", "yellow"] {
        assert_eq!(HighlightColor::from_hex(s), None, "{s}");
    }
}

/// The float form agrees with the hex form. `0x80` is 0.502 and `0xC0` is
/// 0.753 — the values `loki-layout` used to carry as its own copy.
#[test]
fn the_float_form_matches_the_hex_form() {
    let rgb = HighlightColor::DarkRed
        .to_rgb()
        .expect("DarkRed has a colour");
    assert!((rgb.red() - 0.502).abs() < 0.002, "{}", rgb.red());
    assert!(rgb.green().abs() < f32::EPSILON && rgb.blue().abs() < f32::EPSILON);

    let grey = HighlightColor::LightGray
        .to_rgb()
        .expect("LightGray has a colour");
    assert!((grey.red() - 0.753).abs() < 0.002, "{}", grey.red());
}
