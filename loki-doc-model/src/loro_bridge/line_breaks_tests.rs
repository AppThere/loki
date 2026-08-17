// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 AppThere Loki contributors

//! Tests for `line_breaks` — the `\n` ⇄ [`Inline::LineBreak`] round trip.

use super::text_to_inlines;
use crate::content::inline::Inline;

/// The common case must be untouched: no newline, one `Str`, nothing else.
#[test]
fn plain_text_is_a_single_str() {
    assert_eq!(
        text_to_inlines("hello world"),
        vec![Inline::Str("hello world".to_owned())]
    );
    assert_eq!(text_to_inlines(""), vec![Inline::Str(String::new())]);
}

/// A break between two runs of text.
#[test]
fn a_newline_becomes_a_line_break_between_runs() {
    assert_eq!(
        text_to_inlines("a\nb"),
        vec![
            Inline::Str("a".to_owned()),
            Inline::LineBreak,
            Inline::Str("b".to_owned()),
        ]
    );
}

/// Adjacent breaks produce two `LineBreak`s and no empty `Str` between them.
#[test]
fn consecutive_newlines_do_not_emit_empty_runs() {
    assert_eq!(
        text_to_inlines("a\n\nb"),
        vec![
            Inline::Str("a".to_owned()),
            Inline::LineBreak,
            Inline::LineBreak,
            Inline::Str("b".to_owned()),
        ]
    );
    assert!(
        !text_to_inlines("a\n\nb")
            .iter()
            .any(|i| matches!(i, Inline::Str(s) if s.is_empty())),
        "an empty Str would occupy no bytes but still be a caret stop"
    );
}

/// Leading and trailing breaks keep their position and emit no empty runs.
#[test]
fn leading_and_trailing_breaks_are_preserved() {
    assert_eq!(
        text_to_inlines("\na"),
        vec![Inline::LineBreak, Inline::Str("a".to_owned())]
    );
    assert_eq!(
        text_to_inlines("a\n"),
        vec![Inline::Str("a".to_owned()), Inline::LineBreak]
    );
    assert_eq!(text_to_inlines("\n"), vec![Inline::LineBreak]);
}

/// **The invariant the cursor depends on.** Whatever this produces must extract
/// back to exactly the input text, byte for byte — the editor addresses inline
/// content by byte offset, so a reconstruction that added or dropped a
/// character would silently move every caret position after it.
///
/// This mirrors `inlines.rs`'s extraction (`Str` → its bytes, `LineBreak` →
/// one `\n`), which is the function that actually feeds those offsets.
#[test]
fn reconstruction_preserves_the_byte_count() {
    for input in [
        "", "a", "a\nb", "a\n\nb", "\n", "\na", "a\n", "x\ny\nz", "日\n本",
    ] {
        let extracted: String = text_to_inlines(input)
            .iter()
            .map(|i| match i {
                Inline::Str(s) => s.clone(),
                Inline::LineBreak => "\n".to_owned(),
                other => panic!("unexpected inline {other:?}"),
            })
            .collect();
        assert_eq!(
            extracted, input,
            "round trip changed the text for {input:?}"
        );
        assert_eq!(
            extracted.len(),
            input.len(),
            "byte count moved for {input:?} — every caret offset after it would shift"
        );
    }
}
