// SPDX-License-Identifier: Apache-2.0

//! Tests for the metadata dialog's tab set and required-field counting.

use super::*;
use appthere_ui::DialogTabLayout;
use appthere_ui::responsive::Breakpoint;

fn values(pairs: &[(MetaField, &str)]) -> Vec<(MetaField, String)> {
    pairs.iter().map(|(f, v)| (*f, (*v).to_string())).collect()
}

#[test]
fn index_and_from_index_round_trip_for_every_tab() {
    for (i, tab) in MetaTab::ALL.iter().enumerate() {
        assert_eq!(tab.index(), i);
        assert_eq!(MetaTab::from_index(i), *tab);
    }
    assert_eq!(MetaTab::from_index(99), MetaTab::Statistics, "saturates");
}

#[test]
fn labels_match_the_strip_order_and_length() {
    let labels = MetaTab::labels();
    assert_eq!(labels.len(), MetaTab::ALL.len());
    for (i, tab) in MetaTab::ALL.iter().enumerate() {
        assert_eq!(labels[i], tab.label());
    }
}

/// Every editable field must live on exactly one tab. A field on none is
/// unreachable; a field on two edits the same value from two places.
#[test]
fn the_tabs_partition_the_editable_fields() {
    let mut seen: Vec<MetaField> = Vec::new();
    for tab in MetaTab::ALL {
        for field in tab.fields() {
            assert!(!seen.contains(field), "{:?} appears twice", field.label());
            seen.push(*field);
        }
    }
    assert_eq!(
        seen.len(),
        18,
        "all eighteen model fields are reachable from some tab"
    );
}

/// Note 15: General carries the fields most documents actually set, so every
/// EPUB-required field is on the first tab — the one that opens.
#[test]
fn every_required_field_is_on_the_general_tab() {
    for field in EPUB_REQUIRED {
        assert!(
            MetaTab::General.fields().contains(&field),
            "{} must be reachable without hunting",
            field.label()
        );
    }
}

/// The predicate has to be false somewhere, or it is a description rather than
/// a requirement.
#[test]
fn only_the_export_required_fields_are_required() {
    for field in EPUB_REQUIRED {
        assert!(is_epub_required(field));
    }
    for field in [
        MetaField::Subject,
        MetaField::Keywords,
        MetaField::Coverage,
        MetaField::Citation,
        // Warnings in the preflight, not errors: stores ask for these, the
        // specification does not, so the dialog must not badge them required.
        MetaField::Creator,
        MetaField::Publisher,
        // Generated on first save, which is what its own hint says.
        MetaField::Identifier,
    ] {
        assert!(!is_epub_required(field), "{}", field.label());
    }
}

/// The strip's running count: an empty required field is missing, a filled one
/// is not, and whitespace is not a value.
#[test]
fn the_missing_count_tracks_the_required_fields() {
    assert_eq!(missing_required(&[]), EPUB_REQUIRED.len(), "nothing set");

    let all_set = values(&[
        (MetaField::Title, "The long afternoon"),
        (MetaField::Creator, "M. Halloran"),
        (MetaField::Language, "en-GB"),
        (MetaField::Publisher, "Harbour Press"),
    ]);
    assert_eq!(missing_required(&all_set), 0);

    // Blanked on a field that is *still* required — Publisher is a store
    // convention, not a specification requirement, so it no longer counts.
    let mut one_blank = all_set.clone();
    one_blank[0].1 = "   ".to_string();
    assert_eq!(
        missing_required(&one_blank),
        1,
        "whitespace does not satisfy a required field"
    );
}

/// Optional fields do not move the count, or the badge would report a number
/// the user cannot act on.
#[test]
fn optional_fields_do_not_affect_the_missing_count() {
    let base = values(&[
        (MetaField::Title, "T"),
        (MetaField::Creator, "C"),
        (MetaField::Language, "en"),
        (MetaField::Publisher, "P"),
    ]);
    let mut with_optional = base.clone();
    with_optional.push((MetaField::Coverage, String::new()));
    with_optional.push((MetaField::Subject, "Fiction".to_string()));
    assert_eq!(missing_required(&with_optional), missing_required(&base));
}

/// Sentence-shaped fields span the grid; a two-column measure would give them
/// a third of a line.
#[test]
fn only_sentence_fields_are_wide() {
    for field in [
        MetaField::Title,
        MetaField::Description,
        MetaField::Citation,
    ] {
        assert!(MetaTab::is_wide_field(field), "{}", field.label());
    }
    for field in [MetaField::Creator, MetaField::Language, MetaField::Rights] {
        assert!(!MetaTab::is_wide_field(field), "{}", field.label());
    }
}

/// The inline count must genuinely collapse this tab set at Medium.
#[test]
fn the_medium_inline_count_collapses_the_strip() {
    assert!(MetaTab::INLINE_AT_MEDIUM < MetaTab::ALL.len());
    assert_eq!(
        DialogTabLayout::for_breakpoint(
            Breakpoint::Medium,
            MetaTab::ALL.len(),
            MetaTab::INLINE_AT_MEDIUM
        ),
        DialogTabLayout::Overflow {
            visible: 3,
            overflow: 2
        }
    );
}

/// The two derived tabs edit no model field — they are read-only surfaces, and
/// a field assigned to them would never be written back.
#[test]
fn the_derived_tabs_edit_no_fields() {
    assert!(MetaTab::Accessibility.fields().is_empty());
    assert!(MetaTab::Statistics.fields().is_empty());
}
