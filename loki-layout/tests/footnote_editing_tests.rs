// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 AppThere Loki contributors

//! Footnote-body paragraphs must emit *editing data* addressing the live note
//! body: `block_index` = the owning paragraph's block (not the old `0`), and a
//! `PathStep::Note` descent. This is the layout half of editable footnotes
//! (Spec 04 M4, nested-editing increment 3).

use loki_doc_model::PathStep;
use loki_doc_model::content::attr::NodeAttr;
use loki_doc_model::content::block::{Block, StyledParagraph};
use loki_doc_model::content::inline::{Inline, NoteKind};
use loki_doc_model::document::Document;
use loki_layout::{
    DocumentLayout, FontResources, LayoutMode, LayoutOptions, PaginatedLayout, layout_document,
};

fn resources() -> FontResources {
    let mut r = FontResources::new();
    for p in [
        "/usr/share/fonts/truetype/liberation/LiberationSans-Regular.ttf",
        "/usr/share/fonts/truetype/dejavu/DejaVuSans.ttf",
    ] {
        if let Ok(data) = std::fs::read(p) {
            r.register_font(data);
            break;
        }
    }
    r
}

fn styled(inlines: Vec<Inline>) -> Block {
    Block::StyledPara(StyledParagraph {
        style_id: None,
        direct_para_props: None,
        direct_char_props: None,
        inlines,
        attr: NodeAttr::default(),
    })
}

#[test]
fn footnote_body_paragraph_carries_note_editing_path() {
    // Block 0: "see" + a footnote whose body is "the note body".
    let note = Inline::Note(
        NoteKind::Footnote,
        vec![styled(vec![Inline::Str("the note body".into())])],
    );
    let mut doc = Document::new();
    doc.sections[0].blocks = vec![styled(vec![Inline::Str("see ".into()), note])];

    let mut r = resources();
    let DocumentLayout::Paginated(PaginatedLayout { pages, .. }) = layout_document(
        &mut r,
        &doc,
        LayoutMode::Paginated,
        1.0,
        &LayoutOptions {
            preserve_for_editing: true,
            spell: None,
            ..Default::default()
        },
    ) else {
        panic!("paginated layout expected");
    };

    let paras: Vec<_> = pages
        .iter()
        .filter_map(|p| p.editing_data.as_ref())
        .flat_map(|ed| ed.paragraphs.iter())
        .collect();

    // The main paragraph is top-level (block 0, empty path).
    assert!(
        paras
            .iter()
            .any(|p| p.block_index == 0 && p.path.is_empty()),
        "the body paragraph should be present and top-level"
    );

    // The footnote body paragraph carries the note path, owned by block 0.
    let note_para = paras
        .iter()
        .find(|p| !p.path.is_empty())
        .expect("a footnote-body paragraph with a nested editing path");
    assert_eq!(
        note_para.block_index, 0,
        "footnote body must be owned by its reference's block, not 0"
    );
    assert_eq!(
        note_para.path,
        vec![PathStep::Note { note: 0, block: 0 }],
        "footnote body path must address note 0, body block 0"
    );
}
