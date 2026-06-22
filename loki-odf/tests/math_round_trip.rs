// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 AppThere Loki contributors

//! ODT math round-trip: an `Inline::Math` (MathML) is exported as an embedded
//! formula object (`draw:object` → `Object N/content.xml`) and re-imported back
//! to the same MathML.

use std::io::Cursor;

use loki_doc_model::content::block::Block;
use loki_doc_model::content::inline::{Inline, MathType};
use loki_doc_model::document::Document;
use loki_doc_model::io::{DocumentExport, DocumentImport};
use loki_odf::odt::export::OdtExport;
use loki_odf::odt::import::{OdtImport, OdtImportOptions};

const NS: &str = "http://www.w3.org/1998/Math/MathML";

fn doc_with_math(mathml: &str) -> Document {
    let para = Block::Para(vec![
        Inline::Str("x = ".to_string()),
        Inline::Math(MathType::InlineMath, mathml.to_string()),
    ]);
    let mut doc = Document::new();
    doc.sections[0].blocks = vec![para];
    doc
}

fn first_math(doc: &Document) -> Option<String> {
    for section in &doc.sections {
        for block in &section.blocks {
            let inlines = match block {
                Block::Para(i) | Block::Plain(i) => i.as_slice(),
                Block::StyledPara(sp) => sp.inlines.as_slice(),
                _ => &[],
            };
            for i in inlines {
                if let Inline::Math(_, s) = i {
                    return Some(s.clone());
                }
            }
        }
    }
    None
}

fn round_trip(doc: &Document) -> Document {
    let mut buf = Cursor::new(Vec::new());
    OdtExport::export(doc, &mut buf, Default::default()).expect("export should succeed");
    OdtImport::import(Cursor::new(buf.into_inner()), OdtImportOptions::default())
        .expect("re-import should succeed")
}

#[test]
fn embedded_fraction_round_trips() {
    let mathml = format!("<math xmlns=\"{NS}\"><mfrac><mn>1</mn><mn>2</mn></mfrac></math>");
    let re = round_trip(&doc_with_math(&mathml));
    assert_eq!(first_math(&re).as_deref(), Some(mathml.as_str()));
}

#[test]
fn embedded_root_round_trips() {
    let mathml = format!("<math xmlns=\"{NS}\"><mroot><mi>x</mi><mn>3</mn></mroot></math>");
    let re = round_trip(&doc_with_math(&mathml));
    assert_eq!(first_math(&re).as_deref(), Some(mathml.as_str()));
}
