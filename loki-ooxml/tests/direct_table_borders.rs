// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 AppThere Loki contributors

//! A table's **own** `w:tblPr/w:tblBorders` is read, and merges with its
//! style's per edge.
//!
//! `w:tblBorders` was parsed only inside a `w:style`, so a table that declared
//! its borders directly — the shape Word's *Borders* UI writes when the table
//! is not on a bordered style — imported with `borders: None` and drew nothing
//! at all.
//!
//! The resolution rule is measured, not assumed. Exporting
//! `appthere-conformance/fixtures/docx/table-direct-borders.docx` through Word
//! 16.0 and reading the stroked paths back out of the PDF gives:
//!
//! | table | direct set | interior gridlines Word drew |
//! |---|---|---|
//! | A | none (style only) | yes, thin 0.5 pt — presence control |
//! | B | outer only, `insideH/V` **absent** | yes, thin 0.5 pt (from the style), inside 3 pt outer edges |
//! | C | outer + `insideH/V` `w:val="none"` | none — absence control |
//!
//! B is the discriminating case: interior lines survive a direct set that does
//! not mention them, so the merge is **per edge**, not wholesale replacement.
//! C is what forces "absent" and "explicitly none" to stay distinguishable all
//! the way through the mapper — collapsing both to `None`, which the mapper
//! used to do, makes C indistinguishable from B and reinstates the gridlines
//! Word suppressed.

use std::io::Cursor;
use std::path::PathBuf;

use loki_doc_model::content::block::Block;
use loki_doc_model::content::table::core::Table;
use loki_doc_model::document::Document;
use loki_doc_model::io::DocumentExport;
use loki_doc_model::style::props::border::{Border, BorderStyle};
use loki_ooxml::DocxExport;
use loki_ooxml::docx::import::{DocxImportOptions, DocxImporter};

fn fixture() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../appthere-conformance/fixtures/docx/table-direct-borders.docx")
}

fn import() -> Document {
    let bytes = std::fs::read(fixture()).expect("fixture readable");
    load(bytes)
}

fn load(bytes: Vec<u8>) -> Document {
    DocxImporter::new(DocxImportOptions::default())
        .run(Cursor::new(bytes))
        .expect("DOCX imports")
        .document
}

/// The document's three tables, in order (A, B, C).
fn tables(doc: &Document) -> Vec<&Table> {
    doc.sections
        .iter()
        .flat_map(|s| s.blocks.iter())
        .filter_map(|b| match b {
            Block::Table(t) => Some(t.as_ref()),
            _ => None,
        })
        .collect()
}

/// Points, or `None` — the width an edge would draw at.
fn width(e: &Option<Border>) -> Option<f64> {
    e.as_ref().map(|b| b.width.value())
}

/// Whether an edge puts ink on the page: absent draws nothing, and so does an
/// edge explicitly `none` — but only the latter *suppresses* what it is
/// layered over.
fn draws(e: &Option<Border>) -> bool {
    e.as_ref().is_some_and(|b| b.style != BorderStyle::None)
}

#[test]
fn a_table_reads_its_own_tbl_borders() {
    let doc = import();
    let t = tables(&doc);
    assert_eq!(t.len(), 3, "fixture has three tables");

    // A — states none of its own. Not "an empty set": nothing to say, so the
    // style decides. The inversion matters, because a `Some(default)` here
    // would suppress the style's grid entirely once merged.
    assert!(
        t[0].borders.is_none(),
        "a table with no w:tblBorders must state nothing, got {:?}",
        t[0].borders
    );

    // B — four outer edges at 3 pt (w:sz 24 eighth-points), interior absent.
    let b = t[1].borders.as_ref().expect("B declares tblBorders");
    assert_eq!(width(&b.top), Some(3.0));
    assert_eq!(width(&b.left), Some(3.0));
    assert_eq!(width(&b.bottom), Some(3.0));
    assert_eq!(width(&b.right), Some(3.0));
    assert!(
        b.inside_h.is_none() && b.inside_v.is_none(),
        "B's interior edges are absent, not stated"
    );

    // C — same outer edges, interior explicitly `none`. Present (so it
    // suppresses) but drawing nothing. This is the pair of assertions the old
    // mapper failed: it dropped the edge, making C read exactly like B.
    let c = t[2].borders.as_ref().expect("C declares tblBorders");
    assert_eq!(width(&c.top), Some(3.0));
    assert!(
        c.inside_h.is_some() && c.inside_v.is_some(),
        "C states its interior edges as `none` — that is a statement"
    );
    assert!(!draws(&c.inside_h) && !draws(&c.inside_v));
}

/// The whole point of reading the set: what the cells actually resolve to.
///
/// Goes through the same `table_borders_in_force` the layout and ODT export
/// paths use, so this asserts the rule at the level a renderer consumes it —
/// not merely that some bytes were parsed.
#[test]
fn direct_and_style_borders_resolve_the_way_word_drew_them() {
    let doc = import();
    let t = tables(&doc);
    // Every table is 2×2, so cell (0,0)'s bottom/right edges *are* the interior
    // gridlines and its top/left are the outer frame.
    let interior = |tbl: &Table| {
        let set = doc
            .styles
            .table_borders_in_force(tbl)
            .expect("every table resolves a set — all three are on ProbeGrid");
        let (_, right, bottom, _) = set.edges_for(0, 0, 2, 2);
        (right, bottom)
    };
    let outer = |tbl: &Table| {
        let set = doc.styles.table_borders_in_force(tbl).expect("set");
        let (top, _, _, left) = set.edges_for(0, 0, 2, 2);
        (top, left)
    };

    // A: the style alone paints a full thin grid. Presence control — if this
    // fails the other two cases cannot be read as evidence about merging,
    // because the phenomenon is not reachable in the first place.
    let (r, b) = interior(t[0]);
    assert_eq!((width(&r), width(&b)), (Some(0.5), Some(0.5)));
    assert!(draws(&r) && draws(&b));

    // B: the discriminating case. Thick direct outer frame, and the style's
    // thin gridlines *survive* inside it.
    let (top, left) = outer(t[1]);
    assert_eq!((width(&top), width(&left)), (Some(3.0), Some(3.0)));
    let (r, b) = interior(t[1]);
    assert_eq!(
        (width(&r), width(&b)),
        (Some(0.5), Some(0.5)),
        "an absent interior edge falls back to the style's — Word draws it"
    );
    assert!(draws(&r) && draws(&b));

    // C: identical to B but for the explicit `none`, so the difference below is
    // attributable to that alone.
    let (top, left) = outer(t[2]);
    assert_eq!((width(&top), width(&left)), (Some(3.0), Some(3.0)));
    let (r, b) = interior(t[2]);
    assert!(
        !draws(&r) && !draws(&b),
        "an explicit `none` suppresses the style's gridline"
    );
}

/// Export writes the set back into `w:tblPr`, and re-import recovers it.
///
/// Without this, the fix is import-only: opening a directly bordered table and
/// saving it would drop the borders on the way out — the same defect, one
/// direction later.
#[test]
fn direct_tbl_borders_round_trip_through_export() {
    let doc = import();
    let mut out = Cursor::new(Vec::new());
    DocxExport::export(&doc, &mut out, ()).expect("exports");
    let back = load(out.into_inner());

    let before = tables(&doc);
    let after = tables(&back);
    assert_eq!(after.len(), before.len());
    for (i, (b, a)) in before.iter().zip(after.iter()).enumerate() {
        assert_eq!(
            b.borders, a.borders,
            "table {i}'s own border set must survive a round trip"
        );
    }
    // Named explicitly so the loop above cannot pass by comparing None to None
    // for all three: table C's suppressed interior edges are the fragile part,
    // since `w:val=\"nil\"` is what carries them.
    let c = after[2].borders.as_ref().expect("C keeps its set");
    assert!(c.inside_h.is_some(), "explicit `none` must survive export");
    assert!(!draws(&c.inside_h));
}
