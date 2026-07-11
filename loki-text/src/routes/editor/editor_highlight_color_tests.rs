// SPDX-License-Identifier: Apache-2.0

//! Tests for highlight apply/read over a selection.

use loki_doc_model::content::block::Block;
use loki_doc_model::content::inline::Inline;
use loki_doc_model::document::Document;
use loki_doc_model::loro_bridge::{document_to_loro, loro_to_document};
use loki_doc_model::style::props::char_props::HighlightColor;
use loro::LoroDoc;

use super::{apply_highlight, current_highlight};
use crate::editing::cursor::{CursorState, DocumentPosition};

fn loro_with(text: &str) -> LoroDoc {
    let mut doc = Document::new();
    doc.sections[0].blocks = vec![Block::Para(vec![Inline::Str(text.into())])];
    document_to_loro(&doc).expect("to loro")
}

fn selection(start: usize, end: usize) -> CursorState {
    let mut cs = CursorState::new();
    cs.anchor = Some(DocumentPosition::top_level(0, 0, start));
    cs.focus = Some(DocumentPosition::top_level(0, 0, end));
    cs
}

#[test]
fn apply_sets_the_highlight_over_the_selection_only() {
    let loro = loro_with("hello world");
    apply_highlight(&loro, &selection(0, 5), Some("Yellow")).expect("apply");
    assert_eq!(
        current_highlight(&loro, &selection(2, 2)).as_deref(),
        Some("Yellow"),
    );
    assert_eq!(
        current_highlight(&loro, &selection(8, 8)),
        None,
        "untouched outside the selection",
    );
}

#[test]
fn clearing_removes_the_direct_highlight() {
    let loro = loro_with("hello");
    apply_highlight(&loro, &selection(0, 5), Some("Green")).expect("apply");
    apply_highlight(&loro, &selection(0, 5), None).expect("clear");
    assert_eq!(current_highlight(&loro, &selection(2, 2)), None);
}

#[test]
fn highlight_round_trips_into_char_props() {
    let loro = loro_with("hello");
    apply_highlight(&loro, &selection(0, 5), Some("Cyan")).expect("apply");
    let doc = loro_to_document(&loro).expect("rebuild");
    let inlines: &[Inline] = match &doc.sections[0].blocks[0] {
        Block::Para(inlines) => inlines,
        Block::StyledPara(sp) => &sp.inlines,
        other => panic!("unexpected block: {other:?}"),
    };
    let has_cyan = inlines.iter().any(|i| match i {
        Inline::StyledRun(run) => {
            run.direct_props.as_ref().and_then(|p| p.highlight_color) == Some(HighlightColor::Cyan)
        }
        _ => false,
    });
    assert!(has_cyan, "highlight did not round-trip: {inlines:?}");
}
