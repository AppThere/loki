// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 AppThere Loki contributors

//! Tests for formatting-preserving text replacement.

use crate::content::attr::NodeAttr;
use crate::content::block::Block;
use crate::content::inline::{Inline, StyledRun};
use crate::document::Document;
use crate::layout::section::Section;
use crate::loro_bridge::{document_to_loro, loro_to_document};
use crate::loro_mutation::replace_text;
use crate::style::props::char_props::CharProps;
use loki_primitives::color::DocumentColor;

fn red() -> DocumentColor {
    DocumentColor::from_hex("#FF0000").unwrap()
}

fn black() -> DocumentColor {
    DocumentColor::from_hex("#000000").unwrap()
}

fn colored_run(text: &str, color: DocumentColor) -> Inline {
    Inline::StyledRun(StyledRun {
        style_id: None,
        direct_props: Some(Box::new(CharProps {
            color: Some(color),
            ..Default::default()
        })),
        content: vec![Inline::Str(text.to_string())],
        attr: NodeAttr::default(),
    })
}

/// A heading whose first run ("red ") is red and whose second run ("black") is
/// explicitly black — the structure of the ACID heading that regressed when a
/// word in the black part was replaced.
fn red_then_black() -> Document {
    let block = Block::Para(vec![
        colored_run("red ", red()),
        colored_run("black", black()),
    ]);
    let section = Section::with_layout_and_blocks(Default::default(), vec![block]);
    let mut doc = Document::new();
    doc.sections = vec![section];
    doc
}

/// Flattens a paragraph's inlines into `(text, effective_colour)` spans.
fn spans(inlines: &[Inline]) -> Vec<(String, Option<DocumentColor>)> {
    fn walk(
        inlines: &[Inline],
        inherited: Option<DocumentColor>,
        out: &mut Vec<(String, Option<DocumentColor>)>,
    ) {
        for inl in inlines {
            match inl {
                Inline::Str(s) => out.push((s.clone(), inherited.clone())),
                Inline::StyledRun(run) => {
                    let color = run
                        .direct_props
                        .as_ref()
                        .and_then(|p| p.color.clone())
                        .or_else(|| inherited.clone());
                    walk(&run.content, color, out);
                }
                _ => {}
            }
        }
    }
    let mut out = Vec::new();
    walk(inlines, None, &mut out);
    out
}

#[test]
fn replace_preserves_neighbouring_run_colour() {
    let loro = document_to_loro(&red_then_black()).expect("to loro");
    // "red black" → replace the word "black" ([4, 9)) with "blue".
    replace_text(&loro, 0, 4, 5, "blue").expect("replace");

    let doc = loro_to_document(&loro).expect("rebuild");
    let Block::Para(inlines) = &doc.sections[0].blocks[0] else {
        panic!("expected a paragraph");
    };
    let spans = spans(inlines);
    let joined: String = spans.iter().map(|(t, _)| t.as_str()).collect();
    assert_eq!(joined, "red blue", "text replaced");

    // The replacement must keep the replaced word's (black) colour, NOT inherit
    // the preceding run's red.
    let blue = spans
        .iter()
        .find(|(t, _)| t.contains("blue"))
        .expect("blue span present");
    assert_eq!(
        blue.1,
        Some(black()),
        "replacement word must keep the black colour it replaced: {spans:?}"
    );
    // The original red run must still be red.
    let red_span = spans
        .iter()
        .find(|(t, _)| t.contains("red"))
        .expect("red span present");
    assert_eq!(red_span.1, Some(red()), "red run must stay red: {spans:?}");
}

#[test]
fn replace_keeps_a_uniformly_formatted_word_formatted() {
    // Replacing a red word should keep the replacement red.
    let red_run = Inline::StyledRun(StyledRun {
        style_id: None,
        direct_props: Some(Box::new(CharProps {
            color: Some(red()),
            ..Default::default()
        })),
        content: vec![Inline::Str("teh".to_string())],
        attr: NodeAttr::default(),
    });
    let block = Block::Para(vec![red_run]);
    let section = Section::with_layout_and_blocks(Default::default(), vec![block]);
    let mut doc = Document::new();
    doc.sections = vec![section];

    let loro = document_to_loro(&doc).expect("to loro");
    replace_text(&loro, 0, 0, 3, "the").expect("replace");
    let rebuilt = loro_to_document(&loro).expect("rebuild");
    let Block::Para(inlines) = &rebuilt.sections[0].blocks[0] else {
        panic!("para");
    };
    let spans = spans(inlines);
    assert_eq!(spans.len(), 1);
    assert_eq!(spans[0].0, "the");
    assert_eq!(spans[0].1, Some(red()), "replacement keeps the word's red");
}
