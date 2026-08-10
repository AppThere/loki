// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 AppThere Loki contributors

//! DOCX list round-trip (§10 tier 2): `StyledPara.list_id` paragraphs and
//! their nine-level catalog `ListStyle` must survive export and re-import.
//!
//! Ids are not stable across the trip — export assigns `w:numId`s and import
//! names styles after them — so every assertion is structural: membership,
//! levels, and the per-level definitions, not id strings.

use std::io::Cursor;

use loki_doc_model::content::block::{Block, StyledParagraph};
use loki_doc_model::content::inline::Inline;
use loki_doc_model::document::Document;
use loki_doc_model::io::DocumentExport;
use loki_doc_model::style::list_defaults::{
    default_bullet_list_style, default_numbered_list_style,
};
use loki_doc_model::style::list_style::{BulletChar, ListLevelKind, NumberingScheme};
use loki_doc_model::style::props::para_props::ParaProps;
use loki_ooxml::DocxExport;
use loki_ooxml::docx::import::{DocxImportOptions, DocxImporter};

fn export_import(doc: &Document) -> Document {
    let mut buf = Cursor::new(Vec::new());
    DocxExport::export(doc, &mut buf, ()).expect("export should succeed");
    DocxImporter::new(DocxImportOptions::default())
        .run(Cursor::new(buf.into_inner()))
        .expect("re-import should succeed")
        .document
}

/// A styled paragraph that is an item of `list_id` at `level`.
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

/// The `(list_id, list_level)` of a re-imported block, if it is a list item.
fn membership(block: &Block) -> Option<(String, u8)> {
    match block {
        Block::StyledPara(p) => {
            let props = p.direct_para_props.as_ref()?;
            Some((
                props.list_id.as_ref()?.as_str().to_string(),
                props.list_level.unwrap_or(0),
            ))
        }
        _ => None,
    }
}

#[test]
fn nested_bullet_items_survive_with_their_levels() {
    let bullet = default_bullet_list_style();
    let mut doc = Document::new();
    doc.styles
        .list_styles
        .insert(bullet.id.clone(), bullet.clone());
    doc.sections[0].blocks = vec![
        item("top", bullet.id.as_str(), 0),
        item("nested", bullet.id.as_str(), 1),
        item("deep", bullet.id.as_str(), 2),
        item("top again", bullet.id.as_str(), 0),
    ];

    let round = export_import(&doc);
    let members: Vec<_> = round.sections[0]
        .blocks
        .iter()
        .map(|b| membership(b).expect("every item must still be a list item"))
        .collect();

    // One list: all four items share whatever id the trip assigned.
    let ids: Vec<&str> = members.iter().map(|(id, _)| id.as_str()).collect();
    assert!(
        ids.iter().all(|id| *id == ids[0]),
        "items split into different lists: {ids:?}"
    );
    // Levels are the payload the old writer flattened — the inversion that
    // catches a hardcoded single-level serialisation.
    let levels: Vec<u8> = members.iter().map(|(_, l)| *l).collect();
    assert_eq!(levels, vec![0, 1, 2, 0]);

    // The re-imported definition still has all nine levels, still bullets,
    // with the •/○/▪ cycle intact at the top three.
    let (_, style) = round
        .styles
        .list_styles
        .iter()
        .next()
        .expect("catalog must carry the list style");
    assert_eq!(style.levels.len(), 9, "levels were flattened");
    let glyphs: Vec<char> = style
        .levels
        .iter()
        .take(3)
        .map(|l| match &l.kind {
            ListLevelKind::Bullet {
                char: BulletChar::Char(c),
                ..
            } => *c,
            other => panic!("bullet level came back as {other:?}"),
        })
        .collect();
    assert_eq!(glyphs, vec!['•', '○', '▪']);
    // Indent geometry survives (36pt/level → 720 twips → back to 36pt).
    assert!((style.levels[0].indent_start.value() - 36.0).abs() < 0.01);
    assert!((style.levels[1].indent_start.value() - 72.0).abs() < 0.01);
}

#[test]
fn numbered_scheme_cycle_and_formats_survive() {
    let numbered = default_numbered_list_style();
    let mut doc = Document::new();
    doc.styles
        .list_styles
        .insert(numbered.id.clone(), numbered.clone());
    doc.sections[0].blocks = vec![
        item("first", numbered.id.as_str(), 0),
        item("sub a", numbered.id.as_str(), 1),
    ];

    let round = export_import(&doc);
    let (_, style) = round
        .styles
        .list_styles
        .iter()
        .next()
        .expect("catalog must carry the list style");
    assert_eq!(style.levels.len(), 9);
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
}

/// Two different lists must stay two lists — a shared hardcoded numId would
/// merge their numbering.
#[test]
fn distinct_lists_stay_distinct() {
    let bullet = default_bullet_list_style();
    let numbered = default_numbered_list_style();
    let mut doc = Document::new();
    doc.styles
        .list_styles
        .insert(bullet.id.clone(), bullet.clone());
    doc.styles
        .list_styles
        .insert(numbered.id.clone(), numbered.clone());
    doc.sections[0].blocks = vec![
        item("a bullet", bullet.id.as_str(), 0),
        item("a number", numbered.id.as_str(), 0),
    ];

    let round = export_import(&doc);
    let ids: Vec<String> = round.sections[0]
        .blocks
        .iter()
        .map(|b| membership(b).expect("still list items").0)
        .collect();
    assert_ne!(ids[0], ids[1], "the two lists merged");
}

/// A `list_id` the catalog does not define must still export as a list (the
/// default-bullet fallback), not silently degrade to a plain paragraph.
#[test]
fn unknown_list_id_falls_back_to_a_bullet_list() {
    let mut doc = Document::new();
    doc.sections[0].blocks = vec![
        item("orphan one", "no-such-style", 0),
        item("orphan two", "no-such-style", 0),
    ];

    let round = export_import(&doc);
    let members: Vec<_> = round.sections[0]
        .blocks
        .iter()
        .map(|b| membership(b).expect("orphans must stay list items"))
        .collect();
    assert_eq!(
        members[0].0, members[1].0,
        "the orphaned list's items must share one list"
    );
}
