// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 AppThere Loki contributors

//! Tests for the newline → paragraph-break normalisation (`editor_keydown_newline`).
//!
//! The dispatch itself needs a live Loro document and Dioxus signals, so what is
//! pinned here is the pure segmentation that decides **how many** paragraph
//! splits a payload produces and **what text** lands between them.

use super::{carries_newline, paragraph_segments};

/// The predicate must fire on every newline form a `KeyCharacterMap` can hand
/// back, and must *not* fire on ordinary text — the inversion matters, because a
/// predicate that always said "yes" would route every keystroke through the
/// segmenting path.
#[test]
fn carries_newline_detects_every_form_and_no_others() {
    assert!(carries_newline("\n"));
    assert!(carries_newline("\r"));
    assert!(carries_newline("\r\n"));
    assert!(carries_newline("a\nb"));

    assert!(!carries_newline("a"));
    assert!(!carries_newline(""));
    assert!(!carries_newline("hello world"));
    // A literal backslash-n is text, not a newline.
    assert!(!carries_newline("a\\nb"));
}

/// The soft keyboard's Enter: one newline alone must mean exactly one paragraph
/// split and no inserted text.
#[test]
fn a_lone_newline_is_one_split_and_no_text() {
    assert_eq!(
        paragraph_segments("\n"),
        vec!["".to_string(), "".to_string()]
    );
}

/// `n` newlines produce `n + 1` segments — the invariant the caller's
/// "split between adjacent pairs" loop depends on.
#[test]
fn segment_count_is_one_more_than_the_newline_count() {
    for (text, newlines) in [("a", 0), ("a\nb", 1), ("a\nb\nc", 2), ("\n\n", 2)] {
        assert_eq!(
            paragraph_segments(text).len(),
            newlines + 1,
            "{text:?} should yield {} segments",
            newlines + 1
        );
    }
}

/// CRLF is one break, not two. Folding it first is what prevents a Windows-style
/// commit from inserting a stray empty paragraph between every line.
#[test]
fn crlf_is_a_single_break() {
    assert_eq!(
        paragraph_segments("a\r\nb"),
        vec!["a".to_string(), "b".to_string()]
    );
    assert_eq!(
        paragraph_segments("a\rb"),
        vec!["a".to_string(), "b".to_string()]
    );
    assert_eq!(
        paragraph_segments("a\r\nb\r\nc").len(),
        3,
        "two CRLFs are two breaks, not four"
    );
}

/// Text with no newline must round-trip untouched — the fast path's contract.
#[test]
fn plain_text_is_one_untouched_segment() {
    assert_eq!(paragraph_segments("hello"), vec!["hello".to_string()]);
    assert_eq!(paragraph_segments(""), vec!["".to_string()]);
}

/// A trailing newline leaves an empty final segment, which the caller skips —
/// so "para\n" is one paragraph of text followed by one split, not a split with
/// a stray empty insert.
#[test]
fn a_trailing_newline_leaves_an_empty_final_segment() {
    assert_eq!(
        paragraph_segments("para\n"),
        vec!["para".to_string(), "".to_string()]
    );
}
