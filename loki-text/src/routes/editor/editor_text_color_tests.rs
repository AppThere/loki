// SPDX-License-Identifier: Apache-2.0

//! Tests for text-colour apply/read over a selection.

use loki_doc_model::content::block::Block;
use loki_doc_model::content::inline::Inline;
use loki_doc_model::document::Document;
use loki_doc_model::loro_bridge::{document_to_loro, loro_to_document};
use loro::LoroDoc;

use super::{apply_text_color, current_text_color};
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
fn apply_sets_the_colour_over_the_selection_only() {
    let loro = loro_with("hello world");
    apply_text_color(&loro, &selection(0, 5), Some("#C0392B")).expect("apply");
    assert_eq!(
        current_text_color(&loro, &selection(2, 2)).as_deref(),
        Some("#C0392B"),
        "colour applied inside the selection",
    );
    assert_eq!(
        current_text_color(&loro, &selection(8, 8)),
        None,
        "untouched outside the selection",
    );
}

#[test]
fn automatic_removes_the_direct_colour() {
    let loro = loro_with("hello world");
    apply_text_color(&loro, &selection(0, 5), Some("#2980B9")).expect("apply");
    apply_text_color(&loro, &selection(0, 5), None).expect("clear");
    assert_eq!(
        current_text_color(&loro, &selection(2, 2)),
        None,
        "Automatic clears the direct colour mark",
    );
}

#[test]
fn applied_colour_survives_a_round_trip_into_char_props() {
    let loro = loro_with("hello");
    apply_text_color(&loro, &selection(0, 5), Some("#27AE60")).expect("apply");
    let doc = loro_to_document(&loro).expect("rebuild");
    // A colour mark does not change the block type, so the run lives in the
    // paragraph's inlines whether it reads back as a plain or styled paragraph.
    let inlines: &[Inline] = match &doc.sections[0].blocks[0] {
        Block::Para(inlines) => inlines,
        Block::StyledPara(sp) => &sp.inlines,
        other => panic!("unexpected block: {other:?}"),
    };
    // A styled run carries the green colour in its direct char props.
    let has_green = inlines.iter().any(|i| match i {
        Inline::StyledRun(run) => {
            run.direct_props
                .as_ref()
                .and_then(|p| p.color.as_ref())
                .and_then(|c| c.to_hex())
                .as_deref()
                == Some("#27AE60")
        }
        _ => false,
    });
    assert!(
        has_green,
        "colour did not round-trip into char props: {inlines:?}"
    );
}
