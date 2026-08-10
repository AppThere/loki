// SPDX-License-Identifier: Apache-2.0

//! Tests for the span dialog's tab set and provenance levels.

use super::*;
use appthere_ui::DialogTabLayout;
use appthere_ui::responsive::Breakpoint;

#[test]
fn index_and_from_index_round_trip_for_every_tab() {
    for (i, tab) in SpanTab::ALL.iter().enumerate() {
        assert_eq!(tab.index(), i);
        assert_eq!(SpanTab::from_index(i), *tab);
    }
    assert_eq!(SpanTab::from_index(99), SpanTab::Language, "saturates");
}

#[test]
fn labels_match_the_strip_order_and_length() {
    let labels = SpanTab::labels();
    assert_eq!(labels.len(), SpanTab::ALL.len());
    for (i, tab) in SpanTab::ALL.iter().enumerate() {
        assert_eq!(labels[i], tab.label());
    }
}

#[test]
fn the_medium_inline_count_collapses_the_strip() {
    assert!(SpanTab::INLINE_AT_MEDIUM < SpanTab::ALL.len());
    assert_eq!(
        DialogTabLayout::for_breakpoint(
            Breakpoint::Medium,
            SpanTab::ALL.len(),
            SpanTab::INLINE_AT_MEDIUM
        ),
        DialogTabLayout::Overflow {
            visible: 3,
            overflow: 2
        }
    );
}

/// Note 09: the mark wins over every style, the character style over the
/// paragraph style, and the document default is the floor. Each branch must be
/// reachable, or the resolver collapses to one answer.
#[test]
fn resolution_prefers_the_mark_then_the_nearer_style() {
    assert_eq!(
        level_of(true, Some("Emphasis"), Some("Body")),
        SpanLevel::Direct
    );
    assert_eq!(
        level_of(false, Some("Emphasis"), Some("Body")),
        SpanLevel::CharacterStyle
    );
    assert_eq!(
        level_of(false, None, Some("Body")),
        SpanLevel::ParagraphStyle
    );
    assert_eq!(level_of(false, None, None), SpanLevel::Document);
}

/// A mark wins even with no styles at all — otherwise direct formatting on an
/// unstyled run would report as the document default.
#[test]
fn a_mark_wins_with_no_styles_present() {
    assert_eq!(level_of(true, None, None), SpanLevel::Direct);
}

/// Only direct formatting is removed by Clear, so only it offers a Reset.
#[test]
fn only_direct_formatting_is_resettable() {
    assert!(SpanLevel::Direct.is_resettable());
    for level in [
        SpanLevel::CharacterStyle,
        SpanLevel::ParagraphStyle,
        SpanLevel::Document,
    ] {
        assert!(!level.is_resettable(), "{level:?}");
    }
}

/// Direct formatting must map onto the resettable display kind, and the three
/// inherited levels onto non-resettable ones — the shared component decides the
/// accent and the Reset from this.
#[test]
fn levels_map_onto_the_shared_provenance_kinds() {
    assert!(SpanLevel::Direct.kind().is_local());
    for level in [
        SpanLevel::CharacterStyle,
        SpanLevel::ParagraphStyle,
        SpanLevel::Document,
    ] {
        assert!(!level.kind().is_local(), "{level:?}");
    }
}

/// The two style levels read differently: "from character style Emphasis" and
/// "from paragraph style Body" are different sentences, and collapsing them
/// would lose which editor to open.
#[test]
fn the_two_style_levels_produce_distinct_lines() {
    let char_line = SpanLevel::CharacterStyle.text(Some("12 pt"), Some("Emphasis"));
    let para_line = SpanLevel::ParagraphStyle.text(Some("12 pt"), Some("Body"));
    assert_ne!(char_line, para_line);
    assert!(char_line.contains("Emphasis"));
    assert!(para_line.contains("Body"));
}

/// The header states both the selection length and how much of it is direct
/// formatting, so "why is Clear enabled" has an answer on screen.
#[test]
fn the_selection_summary_reports_the_direct_count() {
    let marks = SpanMarks {
        italic: Some(true),
        ..SpanMarks::default()
    };
    let styled = selection_summary(41, Some("Emphasis"), &marks);
    assert!(styled.contains("41"));
    assert!(styled.contains("Emphasis"));

    let plain = selection_summary(41, None, &marks);
    assert!(!plain.contains("Emphasis"));
    assert_ne!(styled, plain);
}
