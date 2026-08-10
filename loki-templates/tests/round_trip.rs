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
        Some("Cousine")
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
        Some("Courier Prime")
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

/// Every template must name only bundled faces (§7.3): a proprietary family
/// raises the substitution chip on any machine without MS fonts, which is the
/// opposite of what a bundled template should do.
#[test]
fn templates_name_only_bundled_faces() {
    for t in loki_templates::TEMPLATES {
        let doc = loki_templates::document(t.id).unwrap();
        for (id, ps) in &doc.styles.paragraph_styles {
            if let Some(font) = ps.char_props.font_name.as_deref() {
                assert!(
                    loki_fonts::is_bundled_family(font),
                    "template {} style {} names non-bundled face {font}",
                    t.id,
                    id.as_str()
                );
            }
        }
    }
}

/// The screenplay next-style chain drives the Enter flow (§7.4): Character →
/// Dialogue → Action (Normal), Transition → SceneHeading, Action → Action.
/// These must survive the `.dotx` round trip, or the flow silently dies.
#[test]
fn screenplay_next_style_chain_round_trips() {
    let doc = loki_templates::document("screenplay").unwrap();
    let next = |id: &str| style(&doc, id).next_style_id.clone();
    assert_eq!(next("Character").as_deref(), Some("Dialogue"));
    assert_eq!(
        next("Dialogue").as_deref(),
        Some("Normal"),
        "speech is followed by action"
    );
    assert_eq!(next("Transition").as_deref(), Some("SceneHeading"));
    assert_eq!(next("Normal").as_deref(), Some("Normal"));
}

/// The screenplay title page ends with a direct page break (§7.1): the first
/// scene heading carries `page_break_before` as a *direct* property, so the
/// script starts on page 2 without breaking before every later scene heading.
///
/// The importer promotes the outline-levelled scene heading to a
/// `Block::Heading`, so the direct break (and the "SceneHeading" style ref)
/// ride the heading's `NodeAttr::kv` — the promotion-preserving channel the
/// layout's heading synthesis reads.
#[test]
fn screenplay_title_page_breaks_before_first_scene() {
    use loki_doc_model::content::block::Block;
    let doc = loki_templates::document("screenplay").unwrap();
    let blocks = &doc.sections[0].blocks;
    let has_break = |b: &Block| match b {
        Block::StyledPara(sp) => sp
            .direct_para_props
            .as_ref()
            .and_then(|pp| pp.page_break_before)
            .unwrap_or(false),
        Block::Heading(_, attr, _) => attr
            .kv
            .iter()
            .any(|(k, v)| k == "page-break-before" && v == "true"),
        _ => false,
    };
    let breaks: Vec<bool> = blocks.iter().map(has_break).collect();
    assert_eq!(
        breaks.iter().filter(|b| **b).count(),
        1,
        "exactly one direct page break (after the title page), got {breaks:?}"
    );
    // The break sits on the first scene heading — the block after the three
    // title-page lines — which keeps its named style through the promotion.
    let first_break = breaks.iter().position(|b| *b);
    assert_eq!(first_break, Some(3));
    match &blocks[3] {
        Block::Heading(1, attr, _) => assert!(
            attr.kv
                .iter()
                .any(|(k, v)| k == "style" && v == "SceneHeading"),
            "promoted heading keeps its SceneHeading style ref, got {:?}",
            attr.kv
        ),
        other => panic!("expected the scene heading block, got {other:?}"),
    }
    // And the style itself carries no break — only the one block does.
    assert_eq!(
        style(&doc, "SceneHeading").para_props.page_break_before,
        None
    );
}

/// Blank = Markdown's catalog minus the sample text (§7.5): same styles, one
/// empty body paragraph, no title. Built programmatically (no asset).
#[test]
fn blank_is_markdown_catalog_with_empty_body() {
    use loki_doc_model::content::block::Block;
    let blank = loki_templates::build_document("blank").unwrap();
    let markdown = loki_templates::build_document("markdown").unwrap();
    let ids = |d: &loki_doc_model::document::Document| -> Vec<String> {
        d.styles
            .paragraph_styles
            .keys()
            .map(|k| k.as_str().to_string())
            .collect()
    };
    assert_eq!(ids(&blank), ids(&markdown), "same catalog as markdown");
    assert_eq!(blank.sections.len(), 1);
    assert_eq!(blank.sections[0].blocks.len(), 1, "one starting paragraph");
    match &blank.sections[0].blocks[0] {
        Block::StyledPara(sp) => {
            assert!(sp.inlines.is_empty(), "no sample text");
            assert_eq!(sp.style_id.as_ref().map(|s| s.as_str()), Some("Normal"));
        }
        other => panic!("expected a styled paragraph, got {other:?}"),
    }
    assert_eq!(
        blank.meta.title, None,
        "untitled until the author titles it"
    );
    // Not a gallery template: no card, no asset.
    assert!(loki_templates::TEMPLATES.iter().all(|t| t.id != "blank"));
    assert!(loki_templates::document("blank").is_none());
}
