// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 AppThere Loki contributors

//! Each bundled `.dotx` asset must import back into a document whose authored
//! styles survived the DOCX round-trip. This guards both the asset bytes and
//! the export/import fidelity the builders rely on.

use loki_doc_model::style::catalog::StyleId;
use loki_doc_model::style::props::para_props::{LineHeight, ParagraphAlignment};

fn style<'a>(
    doc: &'a loki_doc_model::document::Document,
    id: &str,
) -> &'a loki_doc_model::style::para_style::ParagraphStyle {
    doc.styles
        .paragraph_styles
        .get(&StyleId::new(id))
        .unwrap_or_else(|| panic!("style {id} missing after round-trip"))
}

#[test]
fn every_template_id_imports() {
    for t in loki_templates::TEMPLATES {
        assert!(
            loki_templates::document(t.id).is_some(),
            "template {} must import from its bundled asset",
            t.id
        );
        assert!(loki_templates::build_document(t.id).is_some());
    }
    assert!(loki_templates::document("nope").is_none());
}

#[test]
fn markdown_carries_code_and_quote_styles() {
    let doc = loki_templates::document("markdown").unwrap();
    assert_eq!(
        style(&doc, "CodeBlock").char_props.font_name.as_deref(),
        Some("Courier New")
    );
    assert!(style(&doc, "Blockquote").char_props.italic == Some(true));
}

#[test]
fn apa_is_double_spaced_with_first_line_indent() {
    let doc = loki_templates::document("apa").unwrap();
    let normal = style(&doc, "Normal");
    assert!(
        matches!(normal.para_props.line_height, Some(LineHeight::Multiple(m)) if (m - 2.0).abs() < 0.01),
        "APA body must be double-spaced, got {:?}",
        normal.para_props.line_height
    );
    assert_eq!(
        normal
            .para_props
            .indent_first_line
            .map(|p| p.value().round()),
        Some(36.0)
    );
    assert_eq!(
        style(&doc, "Heading1").para_props.alignment,
        Some(ParagraphAlignment::Center)
    );
}

#[test]
fn mla_has_hanging_works_cited() {
    let doc = loki_templates::document("mla").unwrap();
    let wc = style(&doc, "WorksCited");
    assert_eq!(
        wc.para_props.indent_hanging.map(|p| p.value().round()),
        Some(36.0)
    );
}

#[test]
fn screenplay_uses_courier_and_indented_dialogue() {
    let doc = loki_templates::document("screenplay").unwrap();
    assert_eq!(
        style(&doc, "Normal").char_props.font_name.as_deref(),
        Some("Courier New")
    );
    assert_eq!(
        style(&doc, "Dialogue")
            .para_props
            .indent_start
            .map(|p| p.value().round()),
        Some(72.0)
    );
}

#[test]
fn resume_name_is_large_and_bold() {
    let doc = loki_templates::document("resume").unwrap();
    let name = style(&doc, "ResumeName");
    assert_eq!(
        name.char_props.font_size.map(|p| p.value().round()),
        Some(24.0)
    );
    assert_eq!(name.char_props.bold, Some(true));
}
