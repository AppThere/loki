// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 AppThere Loki contributors

use loki_doc_model::content::attr::ExtensionBag;
use loki_doc_model::content::block::Block;
use loki_doc_model::content::inline::Inline;
use loki_doc_model::content::table::col::{ColAlignment, ColSpec, ColWidth};
use loki_doc_model::content::table::core::{Table, TableBody, TableFoot, TableHead};
use loki_doc_model::content::table::row::{Cell, CellProps, Row};
use loki_doc_model::layout::Section;
use loki_doc_model::layout::page::PageLayout;
use loki_doc_model::style::catalog::StyleCatalog;
use loki_primitives::color::DocumentColor;

use loki_layout::{
    FlowOutput, FontResources, LayoutMode, LayoutOptions, PositionedItem, flow_section,
};

/// Flatten positioned items, descending into `ClippedGroup`/`RotatedGroup` so
/// nested cell content is visible to assertions. Table cell content is wrapped
/// in a per-cell `ClippedGroup` (cell-box clip), so a flat scan of the
/// top-level items would otherwise miss every glyph run inside a cell.
fn flatten<'a>(items: &'a [PositionedItem], out: &mut Vec<&'a PositionedItem>) {
    for i in items {
        match i {
            PositionedItem::ClippedGroup { items, .. }
            | PositionedItem::RotatedGroup { items, .. } => flatten(items, out),
            other => out.push(other),
        }
    }
}

fn test_resources() -> FontResources {
    let mut r = FontResources::new();
    for p in ["/usr/share/fonts/truetype/liberation/LiberationSans-Regular.ttf"] {
        if let Ok(data) = std::fs::read(p) {
            r.register_font(data);
        }
    }
    r
}

fn make_para(text: &str) -> loki_doc_model::content::block::StyledParagraph {
    loki_doc_model::content::block::StyledParagraph {
        style_id: None,
        direct_para_props: None,
        direct_char_props: None,
        inlines: vec![Inline::Str(text.into())],
        attr: loki_doc_model::content::attr::NodeAttr::default(),
    }
}

fn make_cell_tall(paras: Vec<&str>, bg_color: Option<DocumentColor>, row_span: u32) -> Cell {
    let props = CellProps {
        background_color: bg_color,
        ..Default::default()
    };
    Cell {
        attr: loki_doc_model::content::attr::NodeAttr::default(),
        alignment: ColAlignment::Default,
        row_span,
        col_span: 1,
        blocks: paras
            .into_iter()
            .map(|p| Block::StyledPara(make_para(p)))
            .collect(),
        props,
    }
}

fn flow_pageless(r: &mut FontResources, section: &Section) -> (Vec<PositionedItem>, f32) {
    let catalog = StyleCatalog::new();
    match flow_section(
        r,
        section,
        &catalog,
        &LayoutMode::Pageless,
        1.0,
        &LayoutOptions::default(),
        &[],
    ) {
        FlowOutput::Canvas { items, height, .. } => (items, height),
        _ => panic!("expected Canvas output"),
    }
}

#[test]
fn test_table_row_height_uniformity() {
    use appthere_color::RgbColor;
    let mut r = test_resources();
    let bg = Some(DocumentColor::Rgb(RgbColor::new(1.0, 0.0, 0.0)));

    // Cell 1: short content (1 paragraph)
    let c1 = make_cell_tall(vec!["Short"], bg.clone(), 1);
    // Cell 2: taller content (3 paragraphs)
    let c2 = make_cell_tall(
        vec!["Tall line 1", "Tall line 2", "Tall line 3"],
        bg.clone(),
        1,
    );

    let row = Row::new(vec![c1, c2]);
    let table = Block::Table(Box::new(Table {
        attr: loki_doc_model::content::attr::NodeAttr::default(),
        caption: Default::default(),
        width: None,
        col_specs: vec![
            ColSpec {
                alignment: ColAlignment::Default,
                width: ColWidth::Default,
            },
            ColSpec {
                alignment: ColAlignment::Default,
                width: ColWidth::Default,
            },
        ],
        head: TableHead::empty(),
        bodies: vec![TableBody::from_rows(vec![row])],
        foot: TableFoot::empty(),
    }));

    let section = Section {
        page_style: None,
        layout: PageLayout::default(),
        start: Default::default(),
        blocks: vec![table],
        extensions: ExtensionBag::default(),
    };

    let (items, _) = flow_pageless(&mut r, &section);
    let bg_rects: Vec<_> = items
        .iter()
        .filter_map(|i| match i {
            PositionedItem::FilledRect(rect) => Some(rect),
            _ => None,
        })
        .collect();

    assert_eq!(bg_rects.len(), 2, "Should have 2 cell background rects");
    let h0 = bg_rects[0].rect.height();
    let h1 = bg_rects[1].rect.height();
    assert!(h0 > 0.0);
    assert_eq!(
        h0, h1,
        "Both cells in the row must have the exact same height"
    );
}

#[test]
fn test_table_row_span_distribution() {
    use appthere_color::RgbColor;
    let mut r = test_resources();
    let bg = Some(DocumentColor::Rgb(RgbColor::new(1.0, 0.0, 0.0)));

    // Row 0:
    // Cell 0: spans 2 rows, has very tall content (5 paragraphs)
    let c00 = make_cell_tall(vec!["P1", "P2", "P3", "P4", "P5"], bg.clone(), 2);
    // Cell 1: row_span 1, short content
    let c01 = make_cell_tall(vec!["Short 1"], bg.clone(), 1);

    // Row 1:
    // Cell 0 is spanned from above (so the row only has 1 cell in row.cells)
    let c11 = make_cell_tall(vec!["Short 2"], bg.clone(), 1);

    let row0 = Row::new(vec![c00, c01]);
    let row1 = Row::new(vec![c11]);

    let table = Block::Table(Box::new(Table {
        attr: loki_doc_model::content::attr::NodeAttr::default(),
        caption: Default::default(),
        width: None,
        col_specs: vec![
            ColSpec {
                alignment: ColAlignment::Default,
                width: ColWidth::Default,
            },
            ColSpec {
                alignment: ColAlignment::Default,
                width: ColWidth::Default,
            },
        ],
        head: TableHead::empty(),
        bodies: vec![TableBody::from_rows(vec![row0, row1])],
        foot: TableFoot::empty(),
    }));

    let section = Section {
        page_style: None,
        layout: PageLayout::default(),
        start: Default::default(),
        blocks: vec![table],
        extensions: ExtensionBag::default(),
    };

    let (items, _) = flow_pageless(&mut r, &section);
    let bg_rects: Vec<_> = items
        .iter()
        .filter_map(|i| match i {
            PositionedItem::FilledRect(rect) => Some(rect),
            _ => None,
        })
        .collect();

    assert_eq!(bg_rects.len(), 3, "Expected 3 cells with backgrounds");

    let rect_c00 = bg_rects
        .iter()
        .find(|r| r.rect.x() < 100.0 && r.rect.y() < 1.0)
        .unwrap();
    let rect_c01 = bg_rects
        .iter()
        .find(|r| r.rect.x() > 100.0 && r.rect.y() < 1.0)
        .unwrap();
    // c11 is in row 1, where column 0 is covered by c00's vertical span, so it
    // must be placed in column 1 (x > 100) — not overlapping the merged cell.
    let rect_c11 = bg_rects
        .iter()
        .find(|r| r.rect.x() > 100.0 && r.rect.y() > 1.0)
        .unwrap();
    assert!(
        (rect_c11.rect.x() - rect_c01.rect.x()).abs() < 1e-3,
        "c11 must sit in the same column as c01 (column 1), not under the vMerge cell"
    );

    let h_c00 = rect_c00.rect.height();
    let h_c01 = rect_c01.rect.height();
    let h_c11 = rect_c11.rect.height();

    assert!(h_c00 > 0.0);
    assert!(h_c01 > 0.0);
    assert!(h_c11 > 0.0);

    assert!(
        (h_c00 - (h_c01 + h_c11)).abs() < 1e-4,
        "Spanning cell height {} must equal sum of spanned row heights ({} + {})",
        h_c00,
        h_c01,
        h_c11
    );
    assert!(
        h_c11 > h_c01,
        "Second row height ({}) should be stretched and be greater than first row height ({})",
        h_c11,
        h_c01
    );
}

#[test]
fn test_table_min_row_height() {
    use appthere_color::RgbColor;
    let mut r = test_resources();
    let bg = Some(DocumentColor::Rgb(RgbColor::new(1.0, 0.0, 0.0)));

    // Create an empty cell (no blocks)
    let c = Cell {
        attr: loki_doc_model::content::attr::NodeAttr::default(),
        alignment: ColAlignment::Default,
        row_span: 1,
        col_span: 1,
        blocks: vec![],
        props: CellProps {
            background_color: bg,
            ..CellProps::default()
        },
    };

    let row = Row::new(vec![c]);
    let table = Block::Table(Box::new(Table {
        attr: loki_doc_model::content::attr::NodeAttr::default(),
        caption: Default::default(),
        width: None,
        col_specs: vec![ColSpec {
            alignment: ColAlignment::Default,
            width: ColWidth::Default,
        }],
        head: TableHead::empty(),
        bodies: vec![TableBody::from_rows(vec![row])],
        foot: TableFoot::empty(),
    }));

    let section = Section {
        page_style: None,
        layout: PageLayout::default(),
        start: Default::default(),
        blocks: vec![table],
        extensions: ExtensionBag::default(),
    };

    let (items, _) = flow_pageless(&mut r, &section);
    let bg_rects: Vec<_> = items
        .iter()
        .filter_map(|i| match i {
            PositionedItem::FilledRect(rect) => Some(rect),
            _ => None,
        })
        .collect();

    assert_eq!(bg_rects.len(), 1);
    assert_eq!(bg_rects[0].rect.height(), loki_layout::MIN_ROW_HEIGHT);
}

#[test]
fn test_table_non_uniform_columns() {
    let mut r = test_resources();
    let bg = Some(DocumentColor::Rgb(appthere_color::RgbColor::new(
        1.0, 0.0, 0.0,
    )));

    let c0 = make_cell_tall(vec!["Col 0"], bg.clone(), 1);
    let c1 = make_cell_tall(vec!["Col 1"], bg.clone(), 1);
    let c2 = make_cell_tall(vec!["Col 2"], bg.clone(), 1);

    let row = Row::new(vec![c0, c1, c2]);
    let table = Block::Table(Box::new(Table {
        attr: loki_doc_model::content::attr::NodeAttr::default(),
        caption: Default::default(),
        width: Some(loki_doc_model::content::table::col::TableWidth::Fixed(
            300.0,
        )),
        col_specs: vec![
            ColSpec {
                alignment: ColAlignment::Default,
                width: ColWidth::Fixed(loki_primitives::units::Points::new(100.0)),
            },
            ColSpec {
                alignment: ColAlignment::Default,
                width: ColWidth::Proportional(1.0),
            },
            ColSpec {
                alignment: ColAlignment::Default,
                width: ColWidth::Proportional(2.0),
            },
        ],
        head: TableHead::empty(),
        bodies: vec![TableBody::from_rows(vec![row])],
        foot: TableFoot::empty(),
    }));

    let section = Section {
        page_style: None,
        layout: PageLayout::default(),
        start: Default::default(),
        blocks: vec![table],
        extensions: ExtensionBag::default(),
    };

    let (items, _) = flow_pageless(&mut r, &section);
    let bg_rects: Vec<_> = items
        .iter()
        .filter_map(|i| match i {
            PositionedItem::FilledRect(rect) => Some(rect),
            _ => None,
        })
        .collect();

    assert_eq!(bg_rects.len(), 3, "Should have 3 cell background rects");

    let w0 = bg_rects[0].rect.width();
    let w1 = bg_rects[1].rect.width();
    let w2 = bg_rects[2].rect.width();

    assert!(
        (w0 - 100.0).abs() < 1e-3,
        "Col 0 width: expected 100, got {}",
        w0
    );
    assert!(
        (w1 - 66.6666).abs() < 1e-1,
        "Col 1 width: expected ~66.7, got {}",
        w1
    );
    assert!(
        (w2 - 133.3333).abs() < 1e-1,
        "Col 2 width: expected ~133.3, got {}",
        w2
    );
}

/// Build an all-fixed-width table: one row, `widths.len()` columns, each a
/// `ColWidth::Fixed`, with an explicit `TableWidth::Fixed(table_width)`.
fn fixed_width_table(widths: &[f64], table_width: f32) -> Block {
    use loki_doc_model::content::table::col::TableWidth;
    let bg = Some(DocumentColor::Rgb(appthere_color::RgbColor::new(
        1.0, 0.0, 0.0,
    )));
    let cells: Vec<Cell> = widths
        .iter()
        .map(|_| make_cell_tall(vec!["x"], bg.clone(), 1))
        .collect();
    let col_specs: Vec<ColSpec> = widths
        .iter()
        .map(|&w| ColSpec {
            alignment: ColAlignment::Default,
            width: ColWidth::Fixed(loki_primitives::units::Points::new(w)),
        })
        .collect();
    Block::Table(Box::new(Table {
        attr: loki_doc_model::content::attr::NodeAttr::default(),
        caption: Default::default(),
        width: Some(TableWidth::Fixed(table_width)),
        col_specs,
        head: TableHead::empty(),
        bodies: vec![TableBody::from_rows(vec![Row::new(cells)])],
        foot: TableFoot::empty(),
    }))
}

fn cell_widths(items: &[PositionedItem]) -> Vec<f32> {
    items
        .iter()
        .filter_map(|i| match i {
            PositionedItem::FilledRect(rect) => Some(rect.rect.width()),
            _ => None,
        })
        .collect()
}

/// CHARACTERIZATION — locks Loki's *current* behaviour, which **diverges from
/// Word**. When the sum of explicit fixed column widths exceeds the table
/// width, Loki scales every column down proportionally to fit. Microsoft Word
/// with `tblLayout w:type="fixed"` instead honours the fixed widths exactly and
/// lets the table overflow the page (clipping/overflowing content).
///
/// This passes today; the `#[ignore]`d test below encodes the target Word
/// behaviour and is unblocked once `w:tblLayout` is parsed and carried into the
/// model. See `docs/fidelity-status.md` (Tables & Images) and the layout audit.
#[test]
fn fixed_columns_overflowing_table_width_are_scaled_down_current_behavior() {
    let mut r = test_resources();
    // 3 columns × 200pt = 600pt of fixed width, but the table declares 300pt.
    let table = fixed_width_table(&[200.0, 200.0, 200.0], 300.0);
    let section = Section {
        page_style: None,
        layout: PageLayout::default(),
        start: Default::default(),
        blocks: vec![table],
        extensions: ExtensionBag::default(),
    };
    let (items, _) = flow_pageless(&mut r, &section);
    let widths = cell_widths(&items);
    assert_eq!(widths.len(), 3, "expected 3 cell rects");
    for (i, w) in widths.iter().enumerate() {
        assert!(
            (w - 100.0).abs() < 1e-2,
            "col {i}: current behaviour scales 200pt → 100pt (300/600); got {w}"
        );
    }
}

/// TARGET SPEC (Word fidelity) — fixed column widths must be honoured exactly;
/// the table is allowed to exceed the declared/table width rather than being
/// rescaled. Ignored until `w:tblLayout="fixed"` is parsed and threaded through
/// `loki-ooxml` → `loki-doc-model` → `loki-layout`. Remove `#[ignore]` once the
/// fixed-layout path exists.
#[test]
fn fixed_columns_should_be_honored_like_word() {
    let mut r = test_resources();
    let mut table = fixed_width_table(&[200.0, 200.0, 200.0], 300.0);
    // Mark the table as fixed-layout (OOXML `w:tblLayout w:type="fixed"`).
    if let Block::Table(t) = &mut table {
        t.attr
            .classes
            .push(loki_doc_model::content::table::core::TABLE_FIXED_LAYOUT_CLASS.to_string());
    }
    let section = Section {
        page_style: None,
        layout: PageLayout::default(),
        start: Default::default(),
        blocks: vec![table],
        extensions: ExtensionBag::default(),
    };
    let (items, _) = flow_pageless(&mut r, &section);
    let widths = cell_widths(&items);
    assert_eq!(widths.len(), 3);
    for (i, w) in widths.iter().enumerate() {
        assert!(
            (w - 200.0).abs() < 1e-2,
            "col {i}: fixed 200pt must be honoured exactly (table overflows); got {w}"
        );
    }
}

/// Cell content must be wrapped in a [`PositionedItem::ClippedGroup`] whose
/// clip rect is the cell's box, so over-wide content cannot bleed into a
/// neighbouring cell (Word clips cell content to the cell boundary). The cell
/// background/border stay *outside* the clip (top-level), so they still paint
/// fully.
#[test]
fn cell_content_is_clipped_to_cell_box() {
    use loki_doc_model::content::table::col::TableWidth;
    use loki_primitives::units::Points;
    let mut r = test_resources();
    let cell = make_cell_tall(vec!["Hello"], None, 1);
    let table = Block::Table(Box::new(Table {
        attr: loki_doc_model::content::attr::NodeAttr::default(),
        caption: Default::default(),
        width: Some(TableWidth::Fixed(120.0)),
        col_specs: vec![ColSpec {
            alignment: ColAlignment::Default,
            width: ColWidth::Fixed(Points::new(120.0)),
        }],
        head: TableHead::empty(),
        bodies: vec![TableBody::from_rows(vec![Row::new(vec![cell])])],
        foot: TableFoot::empty(),
    }));
    let section = Section {
        page_style: None,
        layout: PageLayout::default(),
        start: Default::default(),
        blocks: vec![table],
        extensions: ExtensionBag::default(),
    };
    let (items, _) = flow_pageless(&mut r, &section);

    // Exactly one ClippedGroup wraps the cell's content, and it contains the
    // glyph run(s) — no glyph run leaks to the top level.
    let clip = items
        .iter()
        .find_map(|i| match i {
            PositionedItem::ClippedGroup { clip_rect, items } => Some((clip_rect, items)),
            _ => None,
        })
        .expect("cell content must be wrapped in a ClippedGroup");
    assert!(
        items
            .iter()
            .all(|i| !matches!(i, PositionedItem::GlyphRun(_))),
        "no glyph run may sit at the top level outside the cell clip"
    );
    let (clip_rect, clipped) = clip;
    assert!(
        clipped
            .iter()
            .any(|i| matches!(i, PositionedItem::GlyphRun(_))),
        "the cell's glyph run must be inside the ClippedGroup"
    );
    // The clip rect spans the 120pt column (width), positioned at the left
    // margin (72pt) in pageless layout.
    assert!(
        (clip_rect.size.width - 120.0).abs() < 1.0,
        "clip width should match the 120pt cell; got {}",
        clip_rect.size.width
    );
    assert!(
        clip_rect.size.height > 0.0,
        "clip height must be the row height; got {}",
        clip_rect.size.height
    );
}

/// CHARACTERIZATION — fixed widths that sum to *less* than the table width are
/// also currently scaled (up) to fill the table. Word's behaviour depends on
/// `tblLayout` (fixed: leave a gap; autofit: distribute), so this too is a
/// known divergence pending the `tblLayout` feature.
#[test]
fn fixed_columns_underflowing_table_width_are_scaled_up_current_behavior() {
    let mut r = test_resources();
    // 2 columns × 50pt = 100pt fixed, table declares 300pt → scale ×3 → 150 each.
    let table = fixed_width_table(&[50.0, 50.0], 300.0);
    let section = Section {
        page_style: None,
        layout: PageLayout::default(),
        start: Default::default(),
        blocks: vec![table],
        extensions: ExtensionBag::default(),
    };
    let (items, _) = flow_pageless(&mut r, &section);
    let widths = cell_widths(&items);
    assert_eq!(widths.len(), 2);
    for (i, w) in widths.iter().enumerate() {
        assert!(
            (w - 150.0).abs() < 1e-2,
            "col {i}: current behaviour scales 50pt → 150pt (300/100); got {w}"
        );
    }
}

/// TC-DOCX-003/004/005 L-merge: row 1 has a `vMerge`-restart cell spanning two
/// rows in column 0; the row below has a `gridSpan=2` cell that must land in
/// columns 1–2 (the covered column 0 is skipped), and the merged cell must
/// extend down beside it. Pins the covered-column grid fix.
#[test]
fn vmerge_gridspan_l_merge_places_cells_correctly() {
    use loki_doc_model::content::table::col::TableWidth;
    use loki_primitives::units::Points;
    let mut r = test_resources();
    let bg = Some(DocumentColor::Rgb(appthere_color::RgbColor::new(
        0.5, 0.5, 0.5,
    )));
    let mut header = make_cell_tall(vec!["Header"], bg.clone(), 1);
    header.col_span = 3;
    let a = make_cell_tall(vec!["A"], bg.clone(), 2); // vMerge restart, spans 2 rows
    let b2 = make_cell_tall(vec!["B2"], bg.clone(), 1);
    let c2 = make_cell_tall(vec!["C2"], bg.clone(), 1);
    let mut bc = make_cell_tall(vec!["B3C3"], bg.clone(), 1); // gridSpan=2; continue cell dropped on import
    bc.col_span = 2;

    let table = Block::Table(Box::new(Table {
        attr: Default::default(),
        caption: Default::default(),
        width: Some(TableWidth::Fixed(300.0)),
        col_specs: (0..3)
            .map(|_| ColSpec {
                alignment: ColAlignment::Default,
                width: ColWidth::Fixed(Points::new(100.0)),
            })
            .collect(),
        head: TableHead::empty(),
        bodies: vec![TableBody::from_rows(vec![
            Row::new(vec![header]),
            Row::new(vec![a, b2, c2]),
            Row::new(vec![bc]),
        ])],
        foot: TableFoot::empty(),
    }));
    let section = Section {
        page_style: None,
        layout: PageLayout::default(),
        start: Default::default(),
        blocks: vec![table],
        extensions: ExtensionBag::default(),
    };
    let (items, _) = flow_pageless(&mut r, &section);
    let rects: Vec<_> = items
        .iter()
        .filter_map(|i| match i {
            PositionedItem::FilledRect(rect) => Some(rect.rect),
            _ => None,
        })
        .collect();
    assert_eq!(rects.len(), 5, "5 cell backgrounds (continue cell dropped)");

    let header = rects
        .iter()
        .max_by(|a, b| a.width().partial_cmp(&b.width()).unwrap())
        .unwrap();
    let b3c3 = rects
        .iter()
        .max_by(|a, b| a.y().partial_cmp(&b.y()).unwrap())
        .unwrap();
    // The gridSpan cell skips the vMerge-covered column 0 → starts at column 1.
    assert!(
        (b3c3.x() - header.x() - 100.0).abs() < 1.0,
        "B3C3 must start at column 1 (header.x + 100); got x={} vs header.x={}",
        b3c3.x(),
        header.x()
    );
    assert!(
        (b3c3.width() - 200.0).abs() < 1.0,
        "B3C3 spans columns 1-2 (200pt); got {}",
        b3c3.width()
    );
    // The merged cell (column 0, below the header) extends down beside B3C3.
    let a = rects
        .iter()
        .find(|r| (r.x() - header.x()).abs() < 1.0 && r.y() > header.y() + 1.0)
        .expect("merged cell A in column 0");
    assert!(
        (a.y() + a.height() - (b3c3.y() + b3c3.height())).abs() < 1.0,
        "the vMerge cell must extend to the bottom of the spanned rows"
    );
}

/// A long unbreakable word in a narrow fixed-width cell must wrap *within* the
/// column (CSS `overflow-wrap: anywhere`, matching Word's fixed-layout
/// behaviour) — making the row tall — instead of overflowing horizontally into
/// the neighbouring cell. Pins the TC-DOCX-006 fix.
#[test]
fn long_word_wraps_within_narrow_cell() {
    use loki_doc_model::content::table::col::TableWidth;
    let mut r = test_resources();
    let cell = make_cell_tall(
        vec!["Narrowcolumnshouldnotgrowtofitthislongunbrokenword"],
        None,
        1,
    );
    let table = Block::Table(Box::new(Table {
        attr: loki_doc_model::content::attr::NodeAttr::default(),
        caption: Default::default(),
        width: Some(TableWidth::Fixed(60.0)),
        col_specs: vec![ColSpec {
            alignment: ColAlignment::Default,
            width: ColWidth::Fixed(loki_primitives::units::Points::new(60.0)),
        }],
        head: TableHead::empty(),
        bodies: vec![TableBody::from_rows(vec![Row::new(vec![cell])])],
        foot: TableFoot::empty(),
    }));
    let section = Section {
        page_style: None,
        layout: PageLayout::default(),
        start: Default::default(),
        blocks: vec![table],
        extensions: ExtensionBag::default(),
    };
    let (items, height) = flow_pageless(&mut r, &section);
    let mut flat = Vec::new();
    flatten(&items, &mut flat);

    // Each wrapped line's width must fit the 60pt column (small tolerance), i.e.
    // no line overflows horizontally into a neighbouring cell. (Content sits at
    // the page's left-margin offset, so width — not absolute x — is the check.)
    let max_line_width = flat
        .iter()
        .filter_map(|i| match i {
            PositionedItem::GlyphRun(run) => {
                Some(run.glyphs.iter().map(|g| g.advance).sum::<f32>())
            }
            _ => None,
        })
        .fold(0.0_f32, f32::max);
    assert!(
        max_line_width < 62.0,
        "each wrapped line must fit the 60pt column; widest line = {max_line_width}"
    );
    // The word wrapped across several lines, so the row is much taller than one.
    assert!(
        height > 40.0,
        "the wrapped word must make the row tall; table height = {height}"
    );
    // Multiple glyph runs on distinct baselines confirm the word actually wrapped.
    let distinct_lines = {
        let mut ys: Vec<f32> = flat
            .iter()
            .filter_map(|i| match i {
                PositionedItem::GlyphRun(run) => Some((run.origin.y * 4.0).round()),
                _ => None,
            })
            .collect();
        ys.sort_by(|a, b| a.partial_cmp(b).unwrap());
        ys.dedup();
        ys.len()
    };
    assert!(
        distinct_lines >= 4,
        "expected the long word to wrap to several lines; got {distinct_lines}"
    );
}

#[test]
fn test_table_cell_vertical_alignment() {
    use loki_doc_model::content::table::row::CellVerticalAlign;
    let mut r = test_resources();

    // Column 0: A tall cell with multiple paragraphs to stretch the row height.
    let c0 = make_cell_tall(vec!["Line 1", "Line 2", "Line 3", "Line 4"], None, 1);

    // Column 1: A short cell with middle alignment.
    let mut c1 = make_cell_tall(vec!["Middle"], None, 1);
    c1.props.vertical_align = Some(CellVerticalAlign::Middle);

    // Column 2: A short cell with bottom alignment.
    let mut c2 = make_cell_tall(vec!["Bottom"], None, 1);
    c2.props.vertical_align = Some(CellVerticalAlign::Bottom);

    let row = Row::new(vec![c0, c1, c2]);
    let table = Block::Table(Box::new(Table {
        attr: loki_doc_model::content::attr::NodeAttr::default(),
        caption: Default::default(),
        width: Some(loki_doc_model::content::table::col::TableWidth::Fixed(
            300.0,
        )),
        col_specs: vec![
            ColSpec {
                alignment: ColAlignment::Default,
                width: ColWidth::Fixed(loki_primitives::units::Points::new(100.0)),
            },
            ColSpec {
                alignment: ColAlignment::Default,
                width: ColWidth::Fixed(loki_primitives::units::Points::new(100.0)),
            },
            ColSpec {
                alignment: ColAlignment::Default,
                width: ColWidth::Fixed(loki_primitives::units::Points::new(100.0)),
            },
        ],
        head: TableHead::empty(),
        bodies: vec![TableBody::from_rows(vec![row])],
        foot: TableFoot::empty(),
    }));

    let section = Section {
        page_style: None,
        layout: PageLayout::default(),
        start: Default::default(),
        blocks: vec![table],
        extensions: ExtensionBag::default(),
    };

    let (items, _) = flow_pageless(&mut r, &section);
    let mut flat = Vec::new();
    flatten(&items, &mut flat);

    let glyph_runs: Vec<_> = flat
        .iter()
        .filter_map(|i| match i {
            PositionedItem::GlyphRun(run) => Some(run),
            _ => None,
        })
        .collect();

    // Group runs by their x coordinate (relative to each other: x0 < x1 < x2).
    let run0 = glyph_runs
        .iter()
        .find(|run| run.origin.x < 150.0)
        .expect("col 0 run");
    let run1 = glyph_runs
        .iter()
        .find(|run| run.origin.x >= 150.0 && run.origin.x < 250.0)
        .expect("col 1 run");
    let run2 = glyph_runs
        .iter()
        .find(|run| run.origin.x >= 250.0)
        .expect("col 2 run");

    let y0 = run0.origin.y;
    let y1 = run1.origin.y;
    let y2 = run2.origin.y;

    assert!(
        y0 < y1,
        "Middle-aligned run y ({}) should be below top-aligned run y ({})",
        y1,
        y0
    );
    assert!(
        y1 < y2,
        "Bottom-aligned run y ({}) should be below middle-aligned run y ({})",
        y2,
        y1
    );
}
