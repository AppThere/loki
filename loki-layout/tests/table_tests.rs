// Copyright 2026 AppThere Loki contributors
// SPDX-License-Identifier: Apache-2.0

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
        layout: PageLayout::default(),
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
        layout: PageLayout::default(),
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
    let rect_c11 = bg_rects
        .iter()
        .find(|r| r.rect.x() < 100.0 && r.rect.y() > 1.0)
        .unwrap();

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
        layout: PageLayout::default(),
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
