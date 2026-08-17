// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 AppThere Loki contributors

//! DOCX table-style reference round-trip: a table's named style (`w:tblStyle`,
//! stored in the model as the table's `"style"` attr) must survive export and
//! re-import — the foundation for table banding / conditional formatting
//! (Spec 05, 4a.3).

use std::io::Cursor;

use loki_doc_model::content::block::Block;
use loki_doc_model::content::table::core::Table;
use loki_doc_model::document::Document;
use loki_doc_model::io::DocumentExport;
use loki_ooxml::DocxExport;
use loki_ooxml::docx::import::{DocxImportOptions, DocxImporter};

fn export_import(doc: &Document) -> Document {
    let mut buf = Cursor::new(Vec::new());
    DocxExport::export(doc, &mut buf, ()).expect("export");
    DocxImporter::new(DocxImportOptions::default())
        .run(Cursor::new(buf.into_inner()))
        .expect("re-import")
        .document
}

/// The first `Block::Table` anywhere in the first section's blocks.
fn first_table(doc: &Document) -> Option<&Table> {
    doc.sections[0].blocks.iter().find_map(|b| match b {
        Block::Table(t) => Some(t.as_ref()),
        _ => None,
    })
}

#[test]
fn table_style_reference_round_trips() {
    let mut table = Table::grid(2, 2);
    table.set_style_name(Some("LightGridAccent1".into()));
    let mut doc = Document::new();
    doc.sections[0].blocks = vec![Block::Table(Box::new(table))];

    let back = export_import(&doc);

    let t = first_table(&back).expect("table survives");
    assert_eq!(t.style_name(), Some("LightGridAccent1"));
}

#[test]
fn a_table_without_a_style_stays_unstyled() {
    let mut doc = Document::new();
    doc.sections[0].blocks = vec![Block::Table(Box::new(Table::grid(2, 1)))];

    let back = export_import(&doc);

    assert_eq!(first_table(&back).and_then(Table::style_name), None);
}

#[test]
fn table_style_banding_and_tbllook_round_trip() {
    use loki_doc_model::style::catalog::StyleId;
    use loki_doc_model::style::table_style::{
        TableConditionalFormat, TableLook, TableProps, TableRegion, TableStyle,
    };
    use loki_primitives::color::{DocumentColor, RgbColor};

    let blue = DocumentColor::Rgb(RgbColor::new(
        0x44 as f32 / 255.0,
        0x72 as f32 / 255.0,
        0xC4 as f32 / 255.0,
    ));

    // A banded table style in the catalog.
    let mut style = TableStyle {
        id: StyleId::new("Banded"),
        display_name: Some("Banded".into()),
        parent: None,
        table_props: TableProps {
            row_band_size: Some(2),
            ..TableProps::default()
        },
        conditional: Default::default(),
        extensions: Default::default(),
    };
    style.conditional.insert(
        TableRegion::FirstRow,
        TableConditionalFormat {
            background_color: Some(blue),
            char_props: Default::default(),
        },
    );

    // A table referencing the style, with a non-default look (last row/col on).
    let mut table = Table::grid(2, 2);
    table.set_style_name(Some("Banded".into()));
    let look = TableLook {
        last_row: true,
        last_column: true,
        ..TableLook::default()
    };
    table.set_table_look_code(Some(look.encode_attr()));

    let mut doc = Document::new();
    doc.styles
        .table_styles
        .insert(StyleId::new("Banded"), style);
    doc.sections[0].blocks = vec![Block::Table(Box::new(table))];

    let back = export_import(&doc);

    // The style definition's conditional shading survives.
    let ts = back
        .styles
        .table_styles
        .get(&StyleId::new("Banded"))
        .expect("table style survives");
    assert_eq!(ts.table_props.row_band_size, Some(2));
    let fr = ts
        .conditional
        .get(&TableRegion::FirstRow)
        .and_then(|c| c.background_color.as_ref())
        .expect("firstRow shading survives");
    assert_eq!(fr.to_hex().as_deref(), Some("#4472C4"));

    // The instance's tblLook survives.
    let t = first_table(&back).expect("table survives");
    let back_look =
        TableLook::decode_attr(t.table_look_code().expect("tbllook present")).expect("decodes");
    assert_eq!(back_look, look);
}

/// 4a.3: direct cell borders and padding survive a DOCX round-trip — export
/// previously dropped `w:tcBorders` entirely (the reader parsed them all
/// along), so a bordered cell came back borderless.
#[test]
fn cell_borders_and_padding_round_trip() {
    use loki_doc_model::style::props::border::{Border, BorderStyle};
    use loki_primitives::color::DocumentColor;
    use loki_primitives::units::Points;

    let mut table = Table::grid(1, 2);
    let cell = &mut table.bodies[0].body_rows[0].cells[0];
    cell.props.border_top = Some(Border::solid(
        Points::new(1.0),
        DocumentColor::from_hex("#FF0000").unwrap(),
    ));
    cell.props.border_bottom = Some(Border {
        style: BorderStyle::Dotted,
        width: Points::new(0.5),
        color: None,
        spacing: None,
    });
    cell.props.padding_left = Some(Points::new(9.0));
    let mut doc = Document::new();
    doc.sections[0].blocks = vec![Block::Table(Box::new(table))];

    let back = export_import(&doc);
    let cell = &first_table(&back).expect("table").bodies[0].body_rows[0].cells[0];

    let top = cell.props.border_top.as_ref().expect("top border survives");
    assert_eq!(top.style, BorderStyle::Solid);
    assert!((top.width.value() - 1.0).abs() < 0.01, "1pt width survives");
    assert_eq!(
        top.color
            .as_ref()
            .and_then(DocumentColor::to_hex)
            .as_deref(),
        Some("#FF0000")
    );
    let bottom = cell.props.border_bottom.as_ref().expect("bottom survives");
    assert_eq!(bottom.style, BorderStyle::Dotted);
    assert!((cell.props.padding_left.expect("padding survives").value() - 9.0).abs() < 0.05);
    // The neighbouring plain cell stays clean.
    assert!(
        first_table(&back).unwrap().bodies[0].body_rows[0].cells[1]
            .props
            .border_top
            .is_none()
    );
}

/// 4a.3: the explicit `w:cnfStyle` region mask survives a DOCX round-trip on
/// the cell attr, so re-exported Word tables keep their authoritative region
/// stamps (and the shading resolver keeps preferring them).
#[test]
fn cnf_style_mask_round_trips() {
    let mut table = Table::grid(2, 2);
    table.bodies[0].body_rows[0].cells[0].set_cnf_code(Some("100100001000".into()));
    let mut doc = Document::new();
    doc.sections[0].blocks = vec![Block::Table(Box::new(table))];

    let back = export_import(&doc);
    let t = first_table(&back).expect("table");
    assert_eq!(
        t.bodies[0].body_rows[0].cells[0].cnf_code(),
        Some("100100001000")
    );
    assert_eq!(t.bodies[0].body_rows[0].cells[1].cnf_code(), None);
}

/// 4a.3: a table style's region character formatting (`w:tblStylePr/w:rPr`)
/// survives a DOCX round-trip through the catalog.
#[test]
fn region_char_formatting_round_trips() {
    use loki_doc_model::style::catalog::StyleId;
    use loki_doc_model::style::props::char_props::CharProps;
    use loki_doc_model::style::table_style::{TableConditionalFormat, TableRegion, TableStyle};
    use loki_primitives::units::Points;

    let mut doc = Document::new();
    let mut style = TableStyle {
        id: StyleId::new("HdrBold"),
        display_name: None,
        parent: None,
        table_props: Default::default(),
        conditional: Default::default(),
        extensions: Default::default(),
    };
    style.conditional.insert(
        TableRegion::FirstRow,
        TableConditionalFormat {
            background_color: None,
            char_props: CharProps {
                bold: Some(true),
                font_size: Some(Points::new(14.0)),
                ..Default::default()
            },
        },
    );
    doc.styles
        .table_styles
        .insert(StyleId::new("HdrBold"), style);
    let mut table = Table::grid(2, 2);
    table.set_style_name(Some("HdrBold".into()));
    doc.sections[0].blocks = vec![Block::Table(Box::new(table))];

    let back = export_import(&doc);
    let style = back
        .styles
        .table_styles
        .get(&StyleId::new("HdrBold"))
        .expect("table style survives");
    let hdr = style
        .conditional
        .get(&TableRegion::FirstRow)
        .expect("firstRow region survives without shading");
    assert_eq!(hdr.char_props.bold, Some(true));
    assert_eq!(hdr.char_props.font_size, Some(Points::new(14.0)));
}

/// `w:tblCellMar` on a table style must reach the model, and must be reachable
/// from a *child* style through `w:basedOn`.
///
/// This is how Word actually ships cell margins: the 108-twip left/right
/// default lives on the `w:default="1"` *Normal Table* style, and every real
/// table references a style like *Table Grid* that is `basedOn` it. A flat
/// `catalog.table_styles[name]` lookup returns `None` here and every cell
/// renders flush against its border.
#[test]
fn tbl_cell_mar_resolves_through_the_based_on_chain() {
    use loki_doc_model::style::catalog::StyleId;

    let bytes = std::fs::read("../loki-acid/assets/acid2-docx.docx").expect("fixture readable");
    let doc = DocxImporter::new(DocxImportOptions::default())
        .run(Cursor::new(bytes))
        .expect("import")
        .document;

    // The margins are declared on the parent, not the referenced style — so a
    // test that only checked the referenced style would pass while inheriting
    // nothing.
    let parent = doc
        .styles
        .table_styles
        .get(&StyleId::new("TableNormal"))
        .expect("TableNormal present");
    let own = parent
        .table_props
        .cell_padding
        .as_ref()
        .expect("TableNormal carries w:tblCellMar");
    assert_eq!(own.left.map(|p| p.value()), Some(5.4), "108 twips ÷ 20");
    assert_eq!(own.right.map(|p| p.value()), Some(5.4));
    assert_eq!(
        own.top.map(|p| p.value()),
        Some(0.0),
        "an explicit 0 must survive as 0, not as absent"
    );

    // TableGrid declares none of its own …
    assert!(
        doc.styles
            .table_styles
            .get(&StyleId::new("TableGrid"))
            .expect("TableGrid present")
            .table_props
            .cell_padding
            .is_none(),
        "fixture precondition: TableGrid inherits rather than declares"
    );
    // … but resolves to the parent's through the chain.
    let resolved = doc.styles.table_cell_padding_for(Some("TableGrid"));
    assert_eq!(resolved.left.map(|p| p.value()), Some(5.4));
    assert_eq!(resolved.right.map(|p| p.value()), Some(5.4));
    assert_eq!(resolved.top.map(|p| p.value()), Some(0.0));

    // Guard inversion: an unknown style resolves to nothing rather than
    // borrowing the last style looked up.
    assert!(
        doc.styles
            .table_cell_padding_for(Some("NoSuchStyle"))
            .is_empty(),
        "an unknown style must contribute no padding"
    );
    assert!(doc.styles.table_cell_padding_for(None).is_empty());
}

/// A table style's `w:tblCellMar` must survive a DOCX export→import cycle.
#[test]
fn table_style_cell_margins_round_trip_through_docx() {
    use loki_doc_model::style::catalog::StyleId as SId;
    use loki_doc_model::style::table_padding::CellPadding;
    use loki_doc_model::style::table_style::{TableProps, TableStyle};
    use loki_primitives::units::Points;

    let mut doc = Document::new();
    doc.styles.table_styles.insert(
        SId::new("Padded"),
        TableStyle {
            id: SId::new("Padded"),
            display_name: Some("Padded".into()),
            parent: None,
            table_props: TableProps {
                // Four distinct values so the assertions discriminate *which*
                // side landed where — a uniform value would pass under any
                // permutation of the four tags.
                cell_padding: Some(CellPadding {
                    top: Some(Points::new(1.0)),
                    bottom: Some(Points::new(2.0)),
                    left: Some(Points::new(3.0)),
                    right: Some(Points::new(4.0)),
                }),
                ..Default::default()
            },
            conditional: Default::default(),
            extensions: Default::default(),
        },
    );
    let mut table = Table::grid(2, 2);
    table.set_style_name(Some("Padded".into()));
    doc.sections[0].blocks = vec![Block::Table(Box::new(table))];

    let back = export_import(&doc);
    let p = back
        .styles
        .table_styles
        .get(&SId::new("Padded"))
        .expect("style survives")
        .table_props
        .cell_padding
        .as_ref()
        .expect("w:tblCellMar written and re-read");
    assert_eq!(
        (
            p.top.map(|x| x.value()),
            p.bottom.map(|x| x.value()),
            p.left.map(|x| x.value()),
            p.right.map(|x| x.value())
        ),
        (Some(1.0), Some(2.0), Some(3.0), Some(4.0))
    );
}

/// End-to-end on a real Word document: the `w:tblCellMar` inherited from
/// *Normal Table* must actually move glyphs at layout time.
///
/// The ACID DOCX fixture's three tables are styled *Table Grid*, carry no
/// per-cell `w:tcMar` at all, and inherit top/bottom 0 + left/right 108 twips
/// (5.4pt) from the `w:default="1"` *Normal Table* parent. Before this change
/// their text was laid out flush against the cell edge.
///
/// The DOCX visual-golden axis cannot cover this — no Word goldens are
/// committed yet, so `visual_golden_docx` is a documented no-op — which is why
/// the geometric assertion lives here instead.
#[test]
fn inherited_cell_margins_shift_glyphs_in_the_acid_fixture() {
    use loki_doc_model::io::DocumentImport;
    use loki_layout::{FontResources, LayoutMode, LayoutOptions, PositionedItem, layout_document};
    use loki_ooxml::docx::import::DocxImport;

    let bytes = std::fs::read("../loki-acid/assets/acid2-docx.docx").expect("fixture readable");
    let mut doc =
        DocxImport::import(Cursor::new(bytes), DocxImportOptions::default()).expect("import");
    let mut r = FontResources::new();

    fn glyph_xs(doc: &loki_doc_model::Document, r: &mut FontResources) -> Vec<f32> {
        let l = layout_document(
            r,
            doc,
            LayoutMode::Paginated,
            1.0,
            &LayoutOptions::default(),
        );
        // Cell content is wrapped in a per-cell `ClippedGroup`, so a flat scan
        // of top-level items misses every glyph inside a table — precisely
        // where this change acts. Descend.
        fn walk(items: &[PositionedItem], out: &mut Vec<f32>) {
            for i in items {
                match i {
                    PositionedItem::ClippedGroup { items, .. }
                    | PositionedItem::RotatedGroup { items, .. } => walk(items, out),
                    PositionedItem::GlyphRun(g) => out.push(g.origin.x),
                    _ => {}
                }
            }
        }
        let mut out = Vec::new();
        for i in l.all_items().collect::<Vec<_>>() {
            walk(std::slice::from_ref(i), &mut out);
        }
        out
    }

    let with_margins = glyph_xs(&doc, &mut r);

    // Strip only the style's contribution to recover the pre-fix geometry —
    // the cells themselves are untouched, so any difference is attributable to
    // the inherited `w:tblCellMar` and nothing else.
    for st in doc.styles.table_styles.values_mut() {
        st.table_props.cell_padding = None;
    }
    let without = glyph_xs(&doc, &mut r);

    assert_eq!(
        with_margins.len(),
        without.len(),
        "stripping cell margins must not change how many runs are produced"
    );
    let shifts: Vec<f32> = with_margins
        .iter()
        .zip(&without)
        .map(|(a, b)| a - b)
        .filter(|d| d.abs() > 0.01)
        .collect();

    // Establish the phenomenon is reachable at all before asserting its size:
    // a scan that found nothing would otherwise "pass" the tolerance check.
    assert!(
        !shifts.is_empty(),
        "no glyph moved — the inherited margins never reached layout"
    );
    // Every shift is exactly the specified margin: +5.4 where left-aligned
    // content is pushed in by `fo:padding-left`'s OOXML equivalent, -5.4 where
    // right-aligned content is pulled in by the right margin. Any other
    // magnitude would mean column geometry moved too, which `w:tblCellMar`
    // must not do.
    for d in &shifts {
        assert!(
            (d.abs() - 5.4).abs() < 0.01,
            "every shift must be exactly the inherited 5.4pt margin, got {d}"
        );
    }
    // Both signs must appear: asserting only the magnitude would pass if the
    // right margin were silently dropped and every run moved right.
    assert!(
        shifts.iter().any(|d| *d > 0.0) && shifts.iter().any(|d| *d < 0.0),
        "both the left and the right margin must take effect, got {shifts:?}"
    );
}
