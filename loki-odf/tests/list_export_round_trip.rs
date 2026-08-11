// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 AppThere Loki contributors

//! ODT list export (§10 tier 2) and, since the tier-4 convergence, the full
//! round trip: `StyledPara.list_id` runs export as nested `<text:list>`
//! markup and re-import as the same flat `StyledPara` + `list_id`/`list_level`
//! run — depth and membership are now first-class on both directions.

use std::io::Cursor;

use loki_doc_model::content::block::{Block, StyledParagraph};
use loki_doc_model::content::inline::Inline;
use loki_doc_model::document::Document;
use loki_doc_model::io::{DocumentExport, DocumentImport};
use loki_doc_model::style::list_defaults::{
    default_bullet_list_style, default_numbered_list_style,
};
use loki_doc_model::style::list_style::{ListLevelKind, NumberingScheme};
use loki_doc_model::style::props::para_props::ParaProps;
use loki_odf::odt::export::{OdtExport, OdtExportOptions};
use loki_odf::odt::import::{OdtImport, OdtImportOptions};

fn export(doc: &Document) -> Vec<u8> {
    let mut buf = Cursor::new(Vec::new());
    OdtExport::export(doc, &mut buf, OdtExportOptions::default())
        .expect("ODT export should succeed");
    buf.into_inner()
}

fn import(bytes: Vec<u8>) -> Document {
    OdtImport::import(Cursor::new(bytes), OdtImportOptions::default()).expect("ODT should import")
}

fn item(text: &str, list_id: &str, level: u8) -> Block {
    Block::StyledPara(StyledParagraph {
        style_id: None,
        direct_para_props: Some(Box::new(ParaProps {
            list_id: Some(loki_doc_model::style::list_style::ListId::new(list_id)),
            list_level: Some(level),
            ..Default::default()
        })),
        direct_char_props: None,
        inlines: vec![Inline::Str(text.into())],
        attr: Default::default(),
    })
}

/// The exported `content.xml`, for markup-level assertions.
fn content_xml(bytes: &[u8]) -> String {
    let mut zip = zip::ZipArchive::new(Cursor::new(bytes)).expect("zip");
    let mut file = zip.by_name("content.xml").expect("content.xml");
    let mut s = String::new();
    std::io::Read::read_to_string(&mut file, &mut s).expect("utf8");
    s
}

/// Flattens an inline run to its text.
fn plain_text(inlines: &[Inline]) -> String {
    let mut out = String::new();
    for inline in inlines {
        match inline {
            Inline::Str(t) => out.push_str(t),
            Inline::Space => out.push(' '),
            _ => {}
        }
    }
    out
}

/// Collects every list item in a block sequence as
/// `(text, level, list_id)` — the converged importer emits list items as
/// top-level `StyledPara`s, so this is a flat walk.
fn collect_items(blocks: &[Block], out: &mut Vec<(String, u8, String)>) {
    for block in blocks {
        if let Block::StyledPara(sp) = block
            && let Some(props) = sp.direct_para_props.as_ref()
            && let Some(id) = props.list_id.as_ref()
        {
            out.push((
                plain_text(&sp.inlines),
                props.list_level.unwrap_or(0),
                id.as_str().to_string(),
            ));
        }
    }
}

#[test]
fn nested_run_exports_as_nested_lists_and_reimports_with_depth() {
    let bullet = default_bullet_list_style();
    let mut doc = Document::new();
    doc.styles
        .list_styles
        .insert(bullet.id.clone(), bullet.clone());
    doc.sections[0].blocks = vec![
        item("top", bullet.id.as_str(), 0),
        item("nested", bullet.id.as_str(), 1),
        item("top again", bullet.id.as_str(), 0),
    ];

    let bytes = export(&doc);
    let xml = content_xml(&bytes);
    assert!(
        xml.contains("<text:list style:name=\"__default-bullet\">"),
        "run must open a styled text:list; got: {xml}"
    );
    // The level-1 item nests a list inside the first item rather than closing it.
    assert!(
        xml.contains("<text:list><text:list-item>"),
        "nested item must open a sub-list"
    );

    let round = import(bytes);
    let mut items = Vec::new();
    collect_items(&round.sections[0].blocks, &mut items);
    // The re-imported id is the exported style name itself — with the tier-4
    // convergence, membership survives the trip, not just structure.
    let id = "__default-bullet".to_string();
    assert_eq!(
        items,
        vec![
            ("top".to_string(), 0, id.clone()),
            ("nested".to_string(), 1, id.clone()),
            ("top again".to_string(), 0, id),
        ],
        "nesting depth is the payload the flat writer destroyed"
    );
}

#[test]
fn nine_level_styles_round_trip_through_the_catalog() {
    let numbered = default_numbered_list_style();
    let mut doc = Document::new();
    doc.styles
        .list_styles
        .insert(numbered.id.clone(), numbered.clone());
    doc.sections[0].blocks = vec![item("first", numbered.id.as_str(), 0)];

    let round = import(export(&doc));
    let style = round
        .styles
        .list_styles
        .values()
        .next()
        .expect("catalog must carry the re-imported list style");
    assert_eq!(style.levels.len(), 9, "levels were dropped");

    let schemes: Vec<_> = style
        .levels
        .iter()
        .take(3)
        .map(|l| match &l.kind {
            ListLevelKind::Numbered { scheme, format, .. } => (*scheme, format.clone()),
            other => panic!("numbered level came back as {other:?}"),
        })
        .collect();
    assert_eq!(schemes[0], (NumberingScheme::Decimal, "%1.".to_string()));
    assert_eq!(schemes[1], (NumberingScheme::LowerAlpha, "%2.".to_string()));
    assert_eq!(schemes[2], (NumberingScheme::LowerRoman, "%3.".to_string()));

    // Geometry: 36pt/level indent with an 18pt hanging label survives the
    // space-before/min-label-width encoding.
    assert!((style.levels[0].indent_start.value() - 36.0).abs() < 0.1);
    assert!((style.levels[0].hanging_indent.value() - 18.0).abs() < 0.1);
    assert!((style.levels[1].indent_start.value() - 72.0).abs() < 0.1);
}

/// A list inside a table cell must still be a list (the cell loop groups its
/// run too).
#[test]
fn cell_lists_group_inside_table_cells() {
    use loki_doc_model::content::table::core::Table;

    let bullet = default_bullet_list_style();
    let mut table = Table::grid(1, 1);
    table.bodies[0].body_rows[0].cells[0].blocks = vec![
        item("cell item one", bullet.id.as_str(), 0),
        item("cell item two", bullet.id.as_str(), 0),
    ];
    let mut doc = Document::new();
    doc.styles
        .list_styles
        .insert(bullet.id.clone(), bullet.clone());
    doc.sections[0].blocks = vec![Block::Table(Box::new(table))];

    let xml = content_xml(&export(&doc));
    let cell_start = xml.find("<table:table-cell").expect("cell present");
    let cell_end = xml[cell_start..]
        .find("</table:table-cell>")
        .map(|i| cell_start + i)
        .expect("cell closes");
    let cell = &xml[cell_start..cell_end];
    assert!(
        cell.contains("<text:list style:name=\"__default-bullet\">"),
        "cell run must be wrapped: {cell}"
    );
    // One list, two items — not two single-item lists.
    assert_eq!(cell.matches("<text:list ").count(), 1, "run split: {cell}");
    assert_eq!(cell.matches("<text:list-item>").count(), 2);
}
