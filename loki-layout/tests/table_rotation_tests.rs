// Copyright 2026 AppThere Loki contributors
// SPDX-License-Identifier: Apache-2.0

use loki_doc_model::content::attr::ExtensionBag;
use loki_doc_model::content::block::Block;
use loki_doc_model::content::inline::Inline;
use loki_doc_model::content::table::col::{ColAlignment, ColSpec, ColWidth};
use loki_doc_model::content::table::core::{Table, TableBody, TableFoot, TableHead};
use loki_doc_model::content::table::row::{Cell, CellProps, CellTextDirection, Row};
use loki_doc_model::layout::Section;
use loki_doc_model::layout::page::PageLayout;
use loki_doc_model::style::catalog::StyleCatalog;

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
fn test_table_cell_rotation() {
    let mut r = test_resources();

    // Rotated cell
    let props = CellProps {
        text_direction: Some(CellTextDirection::TbRl),
        ..Default::default()
    };
    let c = Cell {
        attr: loki_doc_model::content::attr::NodeAttr::default(),
        alignment: ColAlignment::Default,
        row_span: 1,
        col_span: 1,
        blocks: vec![Block::StyledPara(make_para("Rotated text"))],
        props,
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
    let rotated_group = items
        .iter()
        .find(|i| matches!(i, PositionedItem::RotatedGroup { .. }));

    assert!(
        rotated_group.is_some(),
        "Expected a RotatedGroup to be generated for rotated cell"
    );
    if let Some(PositionedItem::RotatedGroup { degrees, .. }) = rotated_group {
        assert_eq!(*degrees, 90.0);
    }
}
