// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 AppThere Loki contributors

use std::collections::HashMap;

use loki_doc_model::content::block::Block;

use super::*;
use super::meta;
use super::page_layout::resolve_master_page_name;
use crate::odt::import::OdtImportOptions;
use crate::odt::model::document::{OdfBodyChild, OdfDocument};
use crate::odt::model::paragraph::{OdfParagraph, OdfParagraphChild};
use crate::odt::model::styles::{OdfStyle, OdfStylesheet};
use crate::version::OdfVersion;

fn empty_stylesheet() -> OdfStylesheet {
    OdfStylesheet::default()
}

fn options() -> OdtImportOptions {
    OdtImportOptions::default()
}

fn empty_doc(children: Vec<OdfBodyChild>) -> OdfDocument {
    OdfDocument {
        version: OdfVersion::V1_2,
        version_was_absent: false,
        body_children: children,
    }
}

fn text_paragraph(text: &str, is_heading: bool, level: Option<u8>) -> OdfParagraph {
    OdfParagraph {
        style_name: None,
        outline_level: level,
        is_heading,
        children: vec![OdfParagraphChild::Text(text.into())],
        list_context: None,
    }
}

#[test]
fn empty_document_produces_empty_section() {
    let doc = empty_doc(vec![]);
    let (result, warnings) =
        map_document(&doc, &empty_stylesheet(), None, &HashMap::new(), &options());
    assert!(warnings.is_empty());
    assert_eq!(result.sections.len(), 1);
    assert!(result.sections[0].blocks.is_empty());
}

#[test]
fn heading_is_emitted_as_heading_block() {
    let para = text_paragraph("Title", true, Some(1));
    let doc = empty_doc(vec![OdfBodyChild::Heading(para)]);
    let (result, _) =
        map_document(&doc, &empty_stylesheet(), None, &HashMap::new(), &options());
    let blocks = &result.sections[0].blocks;
    assert_eq!(blocks.len(), 1);
    assert!(
        matches!(blocks[0], Block::Heading(1, _, _)),
        "expected Heading(1), got {:?}",
        blocks[0]
    );
}

#[test]
fn heading_suppressed_when_emit_heading_blocks_false() {
    let para = text_paragraph("Title", true, Some(1));
    let doc = empty_doc(vec![OdfBodyChild::Heading(para)]);
    let opts = OdtImportOptions {
        emit_heading_blocks: false,
        ..options()
    };
    let (result, _) = map_document(&doc, &empty_stylesheet(), None, &HashMap::new(), &opts);
    let blocks = &result.sections[0].blocks;
    assert_eq!(blocks.len(), 1);
    assert!(
        matches!(blocks[0], Block::StyledPara(_)),
        "expected StyledPara, got {:?}",
        blocks[0]
    );
}

#[test]
fn paragraph_is_emitted_as_styled_para() {
    let para = text_paragraph("Hello", false, None);
    let doc = empty_doc(vec![OdfBodyChild::Paragraph(para)]);
    let (result, _) =
        map_document(&doc, &empty_stylesheet(), None, &HashMap::new(), &options());
    let blocks = &result.sections[0].blocks;
    assert!(
        matches!(blocks[0], Block::StyledPara(_)),
        "expected StyledPara, got {:?}",
        blocks[0]
    );
}

#[test]
fn text_content_preserved_in_heading() {
    let para = text_paragraph("Introduction", true, Some(1));
    let doc = empty_doc(vec![OdfBodyChild::Heading(para)]);
    let (result, _) =
        map_document(&doc, &empty_stylesheet(), None, &HashMap::new(), &options());
    if let Block::Heading(_, _, inlines) = &result.sections[0].blocks[0] {
        assert_eq!(inlines.len(), 1);
        assert!(matches!(&inlines[0], loki_doc_model::Inline::Str(s) if s == "Introduction"));
    } else {
        panic!("expected Heading");
    }
}

#[test]
fn meta_title_mapped() {
    use crate::odt::model::document::OdfMeta;
    let odf_meta = OdfMeta {
        title: Some("My Document".into()),
        subject: Some("My Subject".into()),
        creator: Some("Alice".into()),
        initial_creator: Some("Bob".into()),
        keywords: vec!["k1".into(), "k2".into()],
        ..Default::default()
    };
    let doc = empty_doc(vec![]);
    let (result, _) = map_document(
        &doc,
        &empty_stylesheet(),
        Some(&odf_meta),
        &HashMap::new(),
        &options(),
    );
    assert_eq!(result.meta.title.as_deref(), Some("My Document"));
    assert_eq!(result.meta.subject.as_deref(), Some("My Subject"));
    assert_eq!(result.meta.last_modified_by.as_deref(), Some("Alice"));
    assert_eq!(result.meta.creator.as_deref(), Some("Bob"));
    assert_eq!(result.meta.keywords.as_deref(), Some("k1, k2"));
}

#[test]
fn parse_datetime_rfc3339() {
    let dt = meta::parse_datetime("2024-06-15T12:30:00Z");
    assert!(dt.is_some());
}

#[test]
fn parse_datetime_no_tz() {
    let dt = meta::parse_datetime("2024-06-15T12:30:00");
    assert!(dt.is_some());
}

#[test]
fn parse_datetime_invalid_returns_none() {
    let dt = meta::parse_datetime("not-a-date");
    assert!(dt.is_none());
}

// ── resolve_master_page_name unit tests ───────────────────────────────────

fn style_with_mpn(name: &str, mpn: Option<&str>, parent: Option<&str>) -> OdfStyle {
    use crate::odt::model::styles::OdfStyleFamily;
    OdfStyle {
        name: name.into(),
        display_name: None,
        family: OdfStyleFamily::Paragraph,
        parent_name: parent.map(String::from),
        list_style_name: None,
        para_props: None,
        text_props: None,
        col_width: None,
        cell_props: None,
        is_automatic: false,
        master_page_name: mpn.map(String::from),
    }
}

fn make_lookup<'a>(styles: &'a [OdfStyle]) -> HashMap<&'a str, &'a OdfStyle> {
    styles.iter().map(|s| (s.name.as_str(), s)).collect()
}

/// Direct `master_page_name` on the style is returned.
#[test]
fn resolve_mpn_direct() {
    let styles = [style_with_mpn("LandscapeStyle", Some("Landscape"), None)];
    let lookup = make_lookup(&styles);
    assert_eq!(
        resolve_master_page_name("LandscapeStyle", &lookup),
        Some("Landscape".into())
    );
}

/// When the style has no `master_page_name` but its parent does, the
/// parent's value is returned.
#[test]
fn resolve_mpn_inherited_from_parent() {
    let styles = [
        style_with_mpn("Base", Some("Landscape"), None),
        style_with_mpn("Child", None, Some("Base")),
    ];
    let lookup = make_lookup(&styles);
    assert_eq!(
        resolve_master_page_name("Child", &lookup),
        Some("Landscape".into())
    );
}

/// An empty `master_page_name` string is treated as absent — `None` returned.
#[test]
fn resolve_mpn_empty_string_returns_none() {
    let styles = [style_with_mpn("PlainStyle", Some(""), None)];
    let lookup = make_lookup(&styles);
    assert_eq!(resolve_master_page_name("PlainStyle", &lookup), None);
}

/// A style with no master page anywhere in the chain returns `None`.
#[test]
fn resolve_mpn_no_master_page_in_chain() {
    let styles = [
        style_with_mpn("Root", None, None),
        style_with_mpn("Child", None, Some("Root")),
    ];
    let lookup = make_lookup(&styles);
    assert_eq!(resolve_master_page_name("Child", &lookup), None);
}

/// A style that doesn't exist in the lookup returns `None` without panicking.
#[test]
fn resolve_mpn_unknown_style_returns_none() {
    let styles: [OdfStyle; 0] = [];
    let lookup = make_lookup(&styles);
    assert_eq!(resolve_master_page_name("NonExistent", &lookup), None);
}
