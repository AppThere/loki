// SPDX-License-Identifier: Apache-2.0

//! Tests for the span dialog's character-style read/apply path.

use loki_doc_model::content::block::Block;
use loki_doc_model::content::inline::Inline;
use loki_doc_model::document::Document;
use loki_doc_model::loro_bridge::document_to_loro;

use super::super::marks::{OWNED_MARKS, clear_direct};
use super::{apply_char_style, read_char_style};
use crate::editing::cursor::{CursorState, DocumentPosition};

fn loro_with_para(text: &str) -> loro::LoroDoc {
    let mut doc = Document::new();
    doc.sections[0].blocks = vec![Block::Para(vec![Inline::Str(text.into())])];
    document_to_loro(&doc).unwrap()
}

fn selection(from: usize, to: usize) -> CursorState {
    let mut cs = CursorState::new();
    cs.anchor = Some(DocumentPosition::top_level(0, 0, from));
    cs.focus = Some(DocumentPosition::top_level(0, 0, to));
    cs
}

/// Applying a style writes the reference; reading it back at the head of the
/// same selection reports it.
#[test]
fn an_applied_character_style_round_trips() {
    let loro = loro_with_para("hello world");
    let cs = selection(0, 5);
    assert_eq!(read_char_style(&loro, &cs), None);
    apply_char_style(&loro, &cs, &None, &Some("Emphasis".into())).unwrap();
    assert_eq!(read_char_style(&loro, &cs), Some("Emphasis".into()));
}

/// Staging `None` removes the reference — the run falls back to the paragraph
/// level rather than keeping a stale style.
#[test]
fn staging_none_removes_the_reference() {
    let loro = loro_with_para("hello world");
    let cs = selection(0, 5);
    apply_char_style(&loro, &cs, &None, &Some("Emphasis".into())).unwrap();
    apply_char_style(&loro, &cs, &Some("Emphasis".into()), &None).unwrap();
    assert_eq!(read_char_style(&loro, &cs), None);
}

/// An unchanged staging writes nothing — the diff guard, so Apply on an
/// untouched dialog does not stamp a reference onto every run it visits.
#[test]
fn an_unchanged_style_is_not_rewritten() {
    let loro = loro_with_para("hello world");
    // Style only part of the run; then "apply" the unchanged value over a
    // wider selection. A write would spread the style to the whole width.
    apply_char_style(&loro, &selection(0, 5), &None, &Some("Emphasis".into())).unwrap();
    let wide = selection(0, 11);
    apply_char_style(
        &loro,
        &wide,
        &Some("Emphasis".into()),
        &Some("Emphasis".into()),
    )
    .unwrap();
    let tail = selection(6, 11);
    assert_eq!(read_char_style(&loro, &tail), None);
}

/// "Clear direct formatting" must leave the character style in place — it is a
/// style level, not a direct mark (design note 09). This also pins
/// `MARK_CHAR_STYLE_ID` **out** of `OWNED_MARKS`: adding it there would make
/// this fail.
#[test]
fn clear_direct_formatting_leaves_the_character_style() {
    let loro = loro_with_para("hello world");
    let cs = selection(0, 5);
    apply_char_style(&loro, &cs, &None, &Some("Emphasis".into())).unwrap();
    clear_direct(&loro, &cs).unwrap();
    assert_eq!(read_char_style(&loro, &cs), Some("Emphasis".into()));
    assert!(!OWNED_MARKS.contains(&loki_doc_model::MARK_CHAR_STYLE_ID));
}
