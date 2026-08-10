// SPDX-License-Identifier: Apache-2.0

//! Tests for the used-face audit.

use super::*;
use loki_doc_model::content::attr::NodeAttr;
use loki_doc_model::content::inline::LinkTarget;
use loki_doc_model::content::inline::StyledRun;
use loki_doc_model::layout::section::Section;
use loki_doc_model::style::ParagraphStyle;
use loki_doc_model::style::catalog::StyleId;
use loki_doc_model::style::props::CharProps;

fn doc_with(blocks: Vec<Block>) -> Document {
    let mut d = Document::default();
    d.sections = vec![Section {
        blocks,
        ..Default::default()
    }];
    d
}

fn run_with_face(face: &str) -> Inline {
    Inline::StyledRun(StyledRun {
        style_id: None,
        direct_props: Some(Box::new(CharProps {
            font_name: Some(face.to_string()),
            ..Default::default()
        })),
        content: vec![Inline::Str("text".to_string())],
        attr: NodeAttr::default(),
    })
}

#[test]
fn a_document_with_no_faces_lists_none() {
    assert!(used_faces(&doc_with(Vec::new())).is_empty());
}

/// A face used only by a style still ends up in the file, so the table must
/// count style definitions as well as direct runs.
#[test]
fn faces_are_collected_from_styles_and_direct_runs_alike() {
    let mut d = doc_with(vec![Block::Para(vec![run_with_face("Cousine")])]);
    let mut style = ParagraphStyle {
        id: StyleId::new("body"),
        display_name: Some("Body".to_string()),
        parent: None,
        linked_char_style: None,
        para_props: Default::default(),
        char_props: CharProps {
            font_name: Some("Tinos".to_string()),
            ..Default::default()
        },
        next_style_id: None,
        is_default: false,
        is_custom: true,
        extensions: Default::default(),
    };
    style.char_props.font_name = Some("Tinos".to_string());
    d.styles.paragraph_styles.insert(style.id.clone(), style);

    let faces: Vec<String> = used_faces(&d).into_iter().map(|f| f.name).collect();
    assert_eq!(faces, vec!["Cousine".to_string(), "Tinos".to_string()]);
}

/// One family named twice is one row in the table, not two.
#[test]
fn a_face_used_twice_is_listed_once() {
    let d = doc_with(vec![
        Block::Para(vec![run_with_face("Tinos")]),
        Block::Para(vec![run_with_face("Tinos")]),
    ]);
    assert_eq!(used_faces(&d).len(), 1);
}

/// Note 29: only bundled faces can be embedded, and the flag is what the table
/// reports per row.
#[test]
fn bundled_faces_are_embeddable_and_device_faces_are_not() {
    let d = doc_with(vec![Block::Para(vec![
        run_with_face("Tinos"),
        run_with_face("Helvetica Neue"),
    ])]);
    let faces = used_faces(&d);
    assert_eq!(faces.len(), 2);

    let tinos = faces.iter().find(|f| f.name == "Tinos").expect("bundled");
    assert!(tinos.bundled);
    let device = faces
        .iter()
        .find(|f| f.name == "Helvetica Neue")
        .expect("device");
    assert!(!device.bundled);
    assert_eq!(embeddable_count(&faces), 1);
}

/// A blank family name is not a face; listing it would put an empty row in the
/// table.
#[test]
fn blank_family_names_are_ignored() {
    let d = doc_with(vec![Block::Para(vec![
        run_with_face("   "),
        run_with_face("Tinos"),
    ])]);
    assert_eq!(used_faces(&d).len(), 1);
}

/// Faces inside notes, tables and links still reach the file.
#[test]
fn nested_runs_are_collected() {
    let d = doc_with(vec![Block::Para(vec![Inline::Link(
        NodeAttr::default(),
        vec![run_with_face("Carlito")],
        LinkTarget::new("https://example.test/"),
    )])]);
    assert_eq!(
        used_faces(&d)
            .into_iter()
            .map(|f| f.name)
            .collect::<Vec<_>>(),
        vec!["Carlito".to_string()]
    );
}
