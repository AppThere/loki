// SPDX-License-Identifier: Apache-2.0

//! Tests for [`super::BorderEdges`].

use super::*;
use loki_doc_model::loki_primitives::color::DocumentColor;
use loki_doc_model::loki_primitives::units::Points;

fn rule() -> Border {
    Border::solid(
        Points::new(3.0),
        DocumentColor::from_hex("#C85A3A").expect("literal hex parses"),
    )
}

fn style() -> ParagraphStyle {
    ParagraphStyle {
        id: loki_doc_model::style::catalog::StyleId::new("quote"),
        display_name: Some("Quote".to_string()),
        parent: None,
        linked_char_style: None,
        para_props: Default::default(),
        char_props: Default::default(),
        next_style_id: None,
        is_default: false,
        is_custom: true,
        extensions: Default::default(),
    }
}

#[test]
fn an_unset_style_reads_as_no_border() {
    assert_eq!(BorderEdges::of(&style()), BorderEdges::None);
}

/// Round-trip: every selectable preset must read back as itself, or the picker
/// highlights a different option from the one just chosen.
#[test]
fn every_selectable_preset_round_trips_through_the_model() {
    for preset in BorderEdges::SELECTABLE {
        let mut s = style();
        preset.apply(&mut s, rule());
        assert_eq!(BorderEdges::of(&s), preset, "{preset:?} did not round-trip");
    }
}

/// "Start" is a writing-direction term, not a screen edge: in an RTL paragraph
/// the start edge is the right one. Asserting only the LTR case would pass for
/// an implementation that hardcoded `left`.
#[test]
fn the_start_edge_follows_writing_direction() {
    let mut ltr = style();
    BorderEdges::StartOnly.apply(&mut ltr, rule());
    assert!(ltr.para_props.border_left.is_some());
    assert!(ltr.para_props.border_right.is_none());
    assert_eq!(BorderEdges::of(&ltr), BorderEdges::StartOnly);

    let mut rtl = style();
    rtl.para_props.bidi = Some(true);
    BorderEdges::StartOnly.apply(&mut rtl, rule());
    assert!(rtl.para_props.border_right.is_some());
    assert!(rtl.para_props.border_left.is_none());
    assert_eq!(BorderEdges::of(&rtl), BorderEdges::StartOnly);
}

/// A left rule in an RTL paragraph is the *end* edge — not a start rule — so it
/// must report as `Mixed` rather than being mislabelled.
#[test]
fn an_end_edge_rule_is_not_mistaken_for_a_start_rule() {
    let mut rtl = style();
    rtl.para_props.bidi = Some(true);
    rtl.para_props.border_left = Some(rule());
    assert_eq!(BorderEdges::of(&rtl), BorderEdges::Mixed);
}

/// An imported edge set the presets cannot name is reported, not rounded to the
/// nearest preset — rounding would silently delete edges on the next Apply.
#[test]
fn an_unnameable_edge_set_reports_as_mixed() {
    let mut s = style();
    s.para_props.border_top = Some(rule());
    s.para_props.border_bottom = Some(rule());
    assert_eq!(BorderEdges::of(&s), BorderEdges::Mixed);
    assert!(!BorderEdges::Mixed.is_selectable());
    assert_eq!(BorderEdges::Mixed.selectable_index(), None);
}

/// `Mixed` is not selectable, so applying it must leave the imported edges
/// untouched rather than clearing them.
#[test]
fn applying_mixed_is_a_no_op() {
    let mut s = style();
    s.para_props.border_top = Some(rule());
    s.para_props.border_bottom = Some(rule());
    let before = s.clone();

    BorderEdges::Mixed.apply(&mut s, rule());
    assert_eq!(s, before);
}

/// Selecting None must clear every edge, including one the presets could not
/// name — otherwise "no border" leaves a border on screen.
#[test]
fn selecting_none_clears_even_an_unnameable_edge_set() {
    let mut s = style();
    s.para_props.border_top = Some(rule());
    s.para_props.border_bottom = Some(rule());

    BorderEdges::None.apply(&mut s, rule());
    assert_eq!(BorderEdges::of(&s), BorderEdges::None);
    assert!(s.para_props.border_top.is_none());
    assert!(s.para_props.border_bottom.is_none());
}

/// The controls are seeded from the shared edge, so it must be found whichever
/// edges are set — and must be absent when none are.
#[test]
fn the_shared_edge_border_is_found_for_every_preset() {
    assert_eq!(edge_border(&style()), None);
    for preset in [BorderEdges::StartOnly, BorderEdges::All] {
        let mut s = style();
        preset.apply(&mut s, rule());
        assert_eq!(
            edge_border(&s).map(|b| b.width),
            Some(Points::new(3.0)),
            "{preset:?}"
        );
    }
}

/// Every selectable preset has a picker slot, and the indices are distinct —
/// two presets sharing a slot would make one unreachable.
#[test]
fn selectable_presets_map_to_distinct_picker_slots() {
    let mut seen = Vec::new();
    for preset in BorderEdges::SELECTABLE {
        assert!(preset.is_selectable());
        let idx = preset.selectable_index().expect("selectable has an index");
        assert!(!seen.contains(&idx), "{preset:?} shares a slot");
        seen.push(idx);
    }
    assert_eq!(seen.len(), BorderEdges::SELECTABLE.len());
}

/// The bevel styles are deliberately not offered — they need two tones of a
/// colour to read, which Blitz and 1-bit E-Ink cannot paint.
#[test]
fn bevel_border_styles_are_not_offered() {
    for omitted in [
        BorderStyle::Groove,
        BorderStyle::Ridge,
        BorderStyle::Inset,
        BorderStyle::Outset,
    ] {
        assert!(!BORDER_STYLES.contains(&omitted), "{omitted:?}");
    }
    assert!(BORDER_STYLES.contains(&BorderStyle::Solid));
}
