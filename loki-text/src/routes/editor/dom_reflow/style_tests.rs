// SPDX-License-Identifier: Apache-2.0

//! Tests for the DOM reflow view's property → CSS mapping (ADR-0017).

use super::{char_css, css_color, para_css};
use loki_doc_model::loki_primitives::color::DocumentColor;
use loki_doc_model::loki_primitives::units::Points;
use loki_doc_model::style::props::char_props::{CharProps, StrikethroughStyle, UnderlineStyle};
use loki_doc_model::style::props::para_props::{ParaProps, ParagraphAlignment};

/// **An unset property emits nothing.** This is the load-bearing rule of the
/// whole module: the model inherits, and a declaration written for an absent
/// property would override an ancestor that had a real value — silently, and
/// only for documents that use nested runs.
#[test]
fn unset_properties_emit_nothing() {
    assert_eq!(char_css(&CharProps::default()), "");
    assert_eq!(para_css(&ParaProps::default()), "");
}

/// The inverse: a set property does emit, so "" above is about absence and not
/// about the function never working.
#[test]
fn a_set_property_emits_a_declaration() {
    let css = char_css(&CharProps {
        bold: Some(true),
        ..CharProps::default()
    });
    assert!(css.contains("font-weight: bold"), "{css}");
}

/// **`false` is not the same as unset.** A run explicitly not bold, inside a
/// bold ancestor, needs `font-weight: normal` — dropping it because "false is
/// the default" makes the run inherit bold and renders it wrong.
#[test]
fn an_explicit_false_is_emitted_not_dropped() {
    let css = char_css(&CharProps {
        bold: Some(false),
        italic: Some(false),
        ..CharProps::default()
    });
    assert!(css.contains("font-weight: normal"), "{css}");
    assert!(css.contains("font-style: normal"), "{css}");
}

/// **Underline and strikethrough share one CSS property.** Emitting two
/// `text-decoration` declarations makes the second win and the first vanish, so
/// a run that is both would lose its underline.
#[test]
fn underline_and_strikethrough_combine_into_one_declaration() {
    let css = char_css(&CharProps {
        underline: Some(UnderlineStyle::Single),
        strikethrough: Some(StrikethroughStyle::Single),
        ..CharProps::default()
    });
    assert_eq!(
        css.matches("text-decoration").count(),
        1,
        "two text-decoration declarations — the first is dead: {css}"
    );
    assert!(css.contains("underline"), "{css}");
    assert!(css.contains("line-through"), "{css}");
}

/// A family name is quoted, because names contain spaces and an unquoted
/// `Liberation Sans` is two keywords rather than one family.
#[test]
fn a_family_name_is_quoted() {
    let css = char_css(&CharProps {
        font_name: Some("Liberation Sans".into()),
        ..CharProps::default()
    });
    assert!(css.contains("font-family: 'Liberation Sans'"), "{css}");
}

/// **Sizes stay in points.** Converting to px here would restate the 96/72
/// ratio that Blitz already applies, and the two copies would drift.
#[test]
fn sizes_are_emitted_in_points() {
    let css = char_css(&CharProps {
        font_size: Some(Points::new(12.0)),
        ..CharProps::default()
    });
    assert!(css.contains("12pt"), "{css}");
    assert!(!css.contains("px"), "a size was converted to px: {css}");
}

/// Only sRGB colours map; theme and CMYK are dropped rather than approximated.
#[test]
fn only_direct_rgb_colours_map() {
    let rgb = DocumentColor::from_hex("#112233").expect("valid hex");
    assert_eq!(
        css_color(&rgb).as_deref(),
        Some("#112233".to_uppercase().as_str())
    );
    assert!(
        css_color(&DocumentColor::Transparent).is_none(),
        "a colour with no direct sRGB form was approximated"
    );
}

/// Alignment maps to the logical CSS keywords, so a right-to-left document is
/// not laid out with physical directions its script does not use.
#[test]
fn alignment_maps_to_logical_keywords() {
    let of = |a| {
        para_css(&ParaProps {
            alignment: Some(a),
            ..ParaProps::default()
        })
    };
    assert!(of(ParagraphAlignment::Left).contains("text-align: start"));
    assert!(of(ParagraphAlignment::Right).contains("text-align: end"));
    assert!(of(ParagraphAlignment::Center).contains("text-align: center"));
    assert!(of(ParagraphAlignment::Justify).contains("text-align: justify"));
}

/// **A hanging indent and a first-line indent both write `text-indent`, and the
/// model gives first-line the win.** Emitting both would let the hanging value
/// override it, turning an indented first line into an outdented one.
#[test]
fn a_first_line_indent_beats_a_hanging_one() {
    let both = para_css(&ParaProps {
        indent_first_line: Some(Points::new(18.0)),
        indent_hanging: Some(Points::new(36.0)),
        ..ParaProps::default()
    });
    assert_eq!(
        both.matches("text-indent").count(),
        1,
        "two text-indent declarations: {both}"
    );
    assert!(both.contains("text-indent: 18pt"), "{both}");

    // Hanging alone is a negative first-line indent, which is how CSS says it.
    let hanging = para_css(&ParaProps {
        indent_hanging: Some(Points::new(36.0)),
        ..ParaProps::default()
    });
    assert!(hanging.contains("text-indent: -36pt"), "{hanging}");
}

/// Indents use logical properties too, for the same reason as alignment.
#[test]
fn indents_are_logical_and_in_points() {
    let css = para_css(&ParaProps {
        indent_start: Some(Points::new(24.0)),
        indent_end: Some(Points::new(12.0)),
        ..ParaProps::default()
    });
    assert!(css.contains("margin-inline-start: 24pt"), "{css}");
    assert!(css.contains("margin-inline-end: 12pt"), "{css}");
}
