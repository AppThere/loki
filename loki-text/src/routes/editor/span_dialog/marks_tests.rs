// SPDX-License-Identifier: Apache-2.0

//! Tests for the span dialog's direct-formatting model.

use super::*;

fn some_marks() -> SpanMarks {
    SpanMarks {
        italic: Some(true),
        font_size_pt: Some(10.0),
        ..SpanMarks::default()
    }
}

/// A run with no marks carries no direct formatting, and Clear has nothing to
/// do — the state that dims the button.
#[test]
fn a_run_with_no_marks_is_empty() {
    let marks = SpanMarks::default();
    assert!(marks.is_empty());
    assert_eq!(marks.direct_count(), 0);
}

/// Any single mark makes the run directly formatted; the count follows.
#[test]
fn any_mark_makes_the_run_directly_formatted() {
    assert!(!some_marks().is_empty());
    assert_eq!(some_marks().direct_count(), 2);
}

/// `Some(false)` is a direct override that turns a property *off* — it must not
/// read as "unset", or un-bolding a bold character style would look like it did
/// nothing and the Reset affordance would never appear.
#[test]
fn an_explicit_false_is_direct_formatting_not_an_absent_mark() {
    let marks = SpanMarks {
        bold: Some(false),
        ..SpanMarks::default()
    };
    assert!(!marks.is_empty());
    assert_eq!(marks.direct_count(), 1);
}

/// Every field the struct carries is counted. A field added without a line in
/// `direct_count` would be invisible to the header's "n set directly" summary.
#[test]
fn every_field_contributes_to_the_direct_count() {
    let all = SpanMarks {
        font_family: Some("Tinos".to_string()),
        font_size_pt: Some(10.0),
        bold: Some(true),
        italic: Some(true),
        underline: Some("Single".to_string()),
        strikethrough: Some("Single".to_string()),
        small_caps: Some(true),
        all_caps: Some(false),
        color: Some("#1A1A18".to_string()),
        highlight: Some("Yellow".to_string()),
        vertical_align: Some("Superscript".to_string()),
        letter_spacing_pt: Some(0.5),
        language: Some("fr-FR".to_string()),
    };
    assert_eq!(all.direct_count(), OWNED_MARKS.len());
}

/// Clear must remove every mark the dialog can set. A control added without its
/// key in `OWNED_MARKS` would leave formatting the Clear button misses, which
/// is the defect this pairing exists to catch.
#[test]
fn the_owned_mark_list_covers_every_settable_property() {
    let all = SpanMarks {
        font_family: Some("Tinos".to_string()),
        font_size_pt: Some(10.0),
        bold: Some(true),
        italic: Some(true),
        underline: Some("Single".to_string()),
        strikethrough: Some("Single".to_string()),
        small_caps: Some(true),
        all_caps: Some(true),
        color: Some("#000000".to_string()),
        highlight: Some("Yellow".to_string()),
        vertical_align: Some("Superscript".to_string()),
        letter_spacing_pt: Some(0.5),
        language: Some("fr-FR".to_string()),
    };
    assert_eq!(
        all.direct_count(),
        OWNED_MARKS.len(),
        "OWNED_MARKS and SpanMarks disagree on the property set"
    );
}

/// The mark keys must be distinct, or one property would overwrite another.
#[test]
fn owned_mark_keys_are_distinct() {
    let mut seen: Vec<&str> = Vec::new();
    for key in OWNED_MARKS {
        assert!(!seen.contains(&key), "{key} listed twice");
        seen.push(key);
    }
}
