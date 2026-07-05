// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 AppThere Loki contributors

//! Generates the ODF conformance fixture set (Spec 02 §9 / M4) into
//! `appthere-conformance/fixtures/odt/`.
//!
//! Fidelity fixtures reference the **metric-compatible font names directly**
//! (Carlito, Tinos, Gelasio — Spec 02 D4), so reference-app and candidate
//! renders use the identical bundled faces and any diff is a rendering
//! difference, not a substitution disagreement.
//!
//! Run: `cargo run -p loki-odf --example gen_conformance_fixtures`

use std::io::Cursor;

use loki_doc_model::content::attr::NodeAttr;
use loki_doc_model::content::block::Block;
use loki_doc_model::content::inline::{Inline, StyledRun};
use loki_doc_model::document::Document;
use loki_doc_model::io::DocumentExport;
use loki_doc_model::layout::section::Section;
use loki_doc_model::style::props::char_props::CharProps;
use loki_odf::odt::export::{OdtExport, OdtExportOptions};

fn run(text: &str, props: CharProps) -> Inline {
    Inline::StyledRun(StyledRun {
        style_id: None,
        direct_props: Some(Box::new(props)),
        content: vec![Inline::Str(text.into())],
        attr: NodeAttr::default(),
    })
}

fn font(name: &str) -> CharProps {
    CharProps {
        font_name: Some(name.into()),
        ..Default::default()
    }
}

fn doc(blocks: Vec<Block>) -> Document {
    let mut d = Document::default();
    let mut s = Section::new();
    s.blocks = blocks;
    d.sections = vec![s];
    d
}

/// The calibration baseline set: simple single-page documents believed
/// correct in both engines (Spec 02 §7.4).
fn fixtures() -> Vec<(&'static str, Document)> {
    let lorem = "The quick brown fox jumps over the lazy dog, \
                 pack my box with five dozen liquor jugs. ";
    vec![
        (
            "para-carlito",
            doc(vec![
                Block::Para(vec![run(&lorem.repeat(3), font("Carlito"))]),
                Block::Para(vec![run(&lorem.repeat(2), font("Carlito"))]),
            ]),
        ),
        (
            "styles-tinos",
            doc(vec![
                Block::Heading(
                    1,
                    NodeAttr::default(),
                    vec![run("Heading in Tinos", font("Tinos"))],
                ),
                Block::Para(vec![
                    run("Plain, ", font("Tinos")),
                    run(
                        "bold, ",
                        CharProps {
                            bold: Some(true),
                            ..font("Tinos")
                        },
                    ),
                    run(
                        "and italic",
                        CharProps {
                            italic: Some(true),
                            ..font("Tinos")
                        },
                    ),
                    run(" text.", font("Tinos")),
                ]),
            ]),
        ),
        (
            "para-gelasio",
            doc(vec![Block::Para(vec![run(
                &lorem.repeat(2),
                font("Gelasio"),
            )])]),
        ),
    ]
}

fn main() {
    let out_dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../appthere-conformance/fixtures/odt");
    std::fs::create_dir_all(&out_dir).expect("create fixtures dir");
    for (stem, document) in fixtures() {
        let mut buf = Cursor::new(Vec::new());
        OdtExport::export(&document, &mut buf, OdtExportOptions::default()).expect("export");
        let path = out_dir.join(format!("{stem}.odt"));
        std::fs::write(&path, buf.into_inner()).expect("write fixture");
        println!("wrote {}", path.display());
    }
}
