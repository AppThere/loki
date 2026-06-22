// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 AppThere Loki contributors

//! DOCX math round-trip: an `Inline::Math` (MathML) must survive export to OMML
//! and re-import back to the same MathML.

use std::io::Cursor;

use loki_doc_model::content::block::Block;
use loki_doc_model::content::inline::{Inline, MathType};
use loki_doc_model::document::Document;
use loki_doc_model::io::DocumentExport;
use loki_ooxml::DocxExport;
use loki_ooxml::docx::import::{DocxImportOptions, DocxImporter};

const NS: &str = "http://www.w3.org/1998/Math/MathML";

fn doc_with_math(math: Inline) -> Document {
    let para = Block::Para(vec![Inline::Str("x = ".to_string()), math]);
    let mut doc = Document::new();
    doc.sections[0].blocks = vec![para];
    doc
}

/// Returns the first `Inline::Math` found anywhere in the document.
fn first_math(doc: &Document) -> Option<(MathType, String)> {
    for section in &doc.sections {
        for block in &section.blocks {
            let inlines = match block {
                Block::Para(i) | Block::Plain(i) => i.as_slice(),
                Block::StyledPara(sp) => sp.inlines.as_slice(),
                _ => &[],
            };
            for i in inlines {
                if let Inline::Math(t, s) = i {
                    return Some((*t, s.clone()));
                }
            }
        }
    }
    None
}

fn round_trip(doc: &Document) -> Document {
    let mut buf = Cursor::new(Vec::new());
    DocxExport::export(doc, &mut buf, ()).expect("export should succeed");
    DocxImporter::new(DocxImportOptions::default())
        .run(Cursor::new(buf.into_inner()))
        .expect("re-import should succeed")
        .document
}

#[test]
fn inline_fraction_round_trips() {
    let mathml = format!("<math xmlns=\"{NS}\"><mfrac><mn>1</mn><mn>2</mn></mfrac></math>");
    let doc = doc_with_math(Inline::Math(MathType::InlineMath, mathml.clone()));

    let re = round_trip(&doc);
    let (kind, got) = first_math(&re).expect("math must survive round-trip");
    assert_eq!(kind, MathType::InlineMath);
    assert_eq!(got, mathml);
}

#[test]
fn display_superscript_round_trips() {
    // x^2 as display (block) math.
    let mathml = format!("<math xmlns=\"{NS}\"><msup><mi>x</mi><mn>2</mn></msup></math>");
    let doc = doc_with_math(Inline::Math(MathType::DisplayMath, mathml.clone()));

    let re = round_trip(&doc);
    let (kind, got) = first_math(&re).expect("math must survive round-trip");
    assert_eq!(kind, MathType::DisplayMath);
    assert_eq!(got, mathml);
}

#[test]
fn square_root_round_trips() {
    let mathml = format!("<math xmlns=\"{NS}\"><msqrt><mi>x</mi></msqrt></math>");
    let doc = doc_with_math(Inline::Math(MathType::InlineMath, mathml.clone()));

    let re = round_trip(&doc);
    let (_, got) = first_math(&re).expect("math must survive round-trip");
    assert_eq!(got, mathml);
}
