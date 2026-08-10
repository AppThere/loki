// SPDX-License-Identifier: Apache-2.0

//! Tests for [`super::ParaDialogDraft`] and the buffer parse/format pair.

use super::*;
use loki_doc_model::style::catalog::StyleId;
use loki_doc_model::style::props::para_props::{LineHeight, Spacing};

fn style() -> ParagraphStyle {
    ParagraphStyle {
        id: StyleId::new("body-indent"),
        display_name: Some("Body indent".to_string()),
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

/// The distinction the whole dialog rests on: an unset property renders as an
/// **empty** box, never as `0`. A zero would claim the style sets a value it
/// does not, and the provenance line beneath would contradict the control.
#[test]
fn an_unset_measurement_formats_empty_and_a_zero_formats_zero() {
    assert_eq!(fmt_points(None), "");
    assert_eq!(fmt_points(Some(Points::new(0.0))), "0");
    assert_eq!(fmt_u8(None), "");
    assert_eq!(fmt_u8(Some(2)), "2");
}

/// A box reading `18.0` invites the user to re-type the number it already has.
#[test]
fn measurements_render_without_trailing_zeros() {
    assert_eq!(fmt_points(Some(Points::new(18.0))), "18");
    assert_eq!(fmt_points(Some(Points::new(25.4))), "25.4");
    assert_eq!(fmt_points(Some(Points::new(13.5))), "13.5");
    assert_eq!(fmt_points(Some(Points::new(-6.0))), "-6");
}

/// Three outcomes, all distinct: cleared, still-typing, and a value. Folding
/// "not a number yet" into "unset" would drop the property mid-keystroke.
#[test]
fn parsing_separates_cleared_from_still_typing_from_a_value() {
    assert_eq!(parse_points(""), Ok(None), "cleared");
    assert_eq!(parse_points("   "), Ok(None), "cleared");
    assert_eq!(parse_points("-"), Err(()), "still typing a negative");
    assert_eq!(parse_points("abc"), Err(()));
    assert_eq!(parse_points("18"), Ok(Some(Points::new(18.0))));
    assert_eq!(parse_points(" 13.5 "), Ok(Some(Points::new(13.5))));
}

/// Rust's float parser accepts a trailing point, so `1.` commits `1.0` rather
/// than reporting still-typing. That is harmless *only because the buffer keeps
/// the text the user typed*: the next keystroke sees `1.5`, not `1`. If the
/// buffer were ever dropped in favour of re-formatting the model value, the
/// decimal point would vanish under the caret — so this behaviour is pinned
/// here rather than left as an incidental property of `f64::from_str`.
#[test]
fn a_trailing_decimal_point_commits_and_the_buffer_keeps_the_caret_text() {
    assert_eq!(parse_points("1."), Ok(Some(Points::new(1.0))));
    assert_eq!(parse_points("1.5"), Ok(Some(Points::new(1.5))));
}

/// Widow/orphan counts are clamped, and the clamp must actually bite at both
/// ends — a bound that never rejects anything is a description, not a bound.
#[test]
fn line_counts_clamp_at_both_ends() {
    assert_eq!(parse_lines(""), Ok(None));
    assert_eq!(parse_lines("2"), Ok(Some(2)));
    assert_eq!(parse_lines("0"), Ok(Some(1)), "clamped up");
    assert_eq!(parse_lines("200"), Ok(Some(10)), "clamped down");
    assert_eq!(parse_lines("x"), Err(()));
}

/// A zero or negative multiple collapses every line onto one baseline, so it is
/// treated as still-being-typed rather than committed.
#[test]
fn a_non_positive_line_height_multiple_is_never_committed() {
    assert_eq!(parse_multiple(""), Ok(None));
    assert_eq!(parse_multiple("1.35"), Ok(Some(1.35)));
    assert_eq!(parse_multiple("0"), Err(()));
    assert_eq!(parse_multiple("-1"), Err(()));
}

/// Only `Exact` spacing and `Multiple` line heights survive a round-trip
/// through a number box, so the other variants leave it empty rather than
/// showing a value the box would corrupt on the next keystroke.
#[test]
fn unroundtrippable_variants_leave_the_box_empty() {
    assert_eq!(fmt_spacing(Some(&Spacing::Exact(Points::new(6.0)))), "6");
    assert_eq!(fmt_spacing(None), "");
    assert_eq!(
        fmt_line_height(Some(&LineHeight::Multiple(1.35))),
        "1.35",
        "a multiple is editable"
    );
    assert_eq!(
        fmt_line_height(Some(&LineHeight::Exact(Points::new(14.0)))),
        "",
        "an exact line height is not a multiple and must not render as one"
    );
}

/// The buffers must be seeded from the style, or the dialog opens showing empty
/// boxes over a style that has values — which reads as data loss.
#[test]
fn opening_a_draft_seeds_the_buffers_from_the_style() {
    let mut s = style();
    s.char_props.font_size = Some(Points::new(12.0));
    s.para_props.indent_first_line = Some(Points::new(13.5));
    s.para_props.space_after = Some(Spacing::Exact(Points::new(6.0)));
    s.para_props.line_height = Some(LineHeight::Multiple(1.35));
    s.para_props.orphan_control = Some(2);

    let draft = ParaDialogDraft::new(s);
    assert_eq!(draft.buffers.font_size, "12");
    assert_eq!(draft.buffers.indent_first, "13.5");
    assert_eq!(draft.buffers.space_after, "6");
    assert_eq!(draft.buffers.line_height, "1.35");
    assert_eq!(draft.buffers.orphan, "2");
    assert_eq!(draft.buffers.indent_start, "", "unset stays empty");
}

/// A freshly-opened draft has nothing staged, so Apply is inert and cannot pin
/// inherited values by accident.
#[test]
fn a_fresh_draft_is_clean_and_any_property_change_dirties_it() {
    let mut draft = ParaDialogDraft::new(style());
    assert!(!draft.is_dirty());

    draft.style.para_props.indent_first_line = Some(Points::new(13.5));
    assert!(draft.is_dirty());
}

/// Reset-all clears what the style *sets* without changing what it *is*: an
/// identity change here would rename or re-parent the style behind the user.
#[test]
fn reset_all_clears_properties_but_keeps_identity() {
    let mut s = style();
    s.parent = Some(StyleId::new("body"));
    s.next_style_id = Some("body-indent".to_string());
    s.char_props.font_size = Some(Points::new(12.0));
    s.para_props.indent_first_line = Some(Points::new(13.5));
    s.para_props.keep_with_next = Some(true);

    let mut draft = ParaDialogDraft::new(s);
    draft.reset_all_to_inherited();

    assert_eq!(draft.style.char_props.font_size, None);
    assert_eq!(draft.style.para_props.indent_first_line, None);
    assert_eq!(draft.style.para_props.keep_with_next, None);
    assert_eq!(draft.style.id, StyleId::new("body-indent"));
    assert_eq!(draft.style.display_name.as_deref(), Some("Body indent"));
    assert_eq!(draft.style.parent, Some(StyleId::new("body")));
    assert_eq!(draft.style.next_style_id.as_deref(), Some("body-indent"));
}

/// Reset-all must also empty the boxes; leaving `12` in a box whose property is
/// now `None` shows the user a value the style no longer sets.
#[test]
fn reset_all_empties_the_buffers_and_stays_dirty_until_applied() {
    let mut s = style();
    s.char_props.font_size = Some(Points::new(12.0));
    let mut draft = ParaDialogDraft::new(s);

    draft.reset_all_to_inherited();
    assert_eq!(draft.buffers.font_size, "");
    assert!(
        draft.is_dirty(),
        "the reset itself is a staged change until Apply commits it"
    );
}

/// Resetting a style that already set nothing is a no-op, not a spurious edit —
/// otherwise Apply lights up on a dialog the user only looked at.
#[test]
fn resetting_an_already_inherited_style_stages_nothing() {
    let mut draft = ParaDialogDraft::new(style());
    draft.reset_all_to_inherited();
    assert!(!draft.is_dirty());
}
