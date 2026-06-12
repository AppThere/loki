// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 AppThere Loki contributors

//! Unit tests for cell property mapping.

use super::helpers::make_ctx;
use crate::docx::import::DocxImportOptions;
use crate::docx::model::styles::{DocxTableCell, DocxTcPr};
use crate::docx::mapper::table::cell::map_cell;
use loki_doc_model::content::block::Block;
use loki_doc_model::style::catalog::StyleCatalog;
use loki_opc::PartData;
use std::collections::HashMap;

fn cell_with_props(tc_pr: DocxTcPr) -> DocxTableCell {
    DocxTableCell {
        tc_pr: Some(tc_pr),
        paragraphs: vec![],
    }
}

#[test]
fn cell_padding_maps_to_points() {
    use crate::docx::model::styles::DocxCellMargins;
    use loki_primitives::units::Points;

    let styles = StyleCatalog::default();
    let (fn_m, en_m, hl_m, img_m) = (
        HashMap::new(),
        HashMap::new(),
        HashMap::new(),
        HashMap::new(),
    );
    let opts = DocxImportOptions::default();
    let mut ctx = make_ctx(&styles, &fn_m, &en_m, &hl_m, &img_m, &opts);

    let tc = cell_with_props(DocxTcPr {
        tc_margins: Some(DocxCellMargins {
            top: Some(100),    // 5pt
            bottom: Some(200), // 10pt
            left: Some(300),   // 15pt
            right: Some(400),  // 20pt
        }),
        ..Default::default()
    });
    let cell = map_cell(&tc, &mut ctx);
    assert_eq!(cell.props.padding_top, Some(Points::new(5.0)));
    assert_eq!(cell.props.padding_bottom, Some(Points::new(10.0)));
    assert_eq!(cell.props.padding_left, Some(Points::new(15.0)));
    assert_eq!(cell.props.padding_right, Some(Points::new(20.0)));
}

#[test]
fn cell_valign_maps_correctly() {
    use crate::docx::model::styles::DocxVAlign;
    use loki_doc_model::content::table::row::CellVerticalAlign;

    let styles = StyleCatalog::default();
    let (fn_m, en_m, hl_m, img_m) = (
        HashMap::new(),
        HashMap::new(),
        HashMap::new(),
        HashMap::new(),
    );
    let opts = DocxImportOptions::default();
    let mut ctx = make_ctx(&styles, &fn_m, &en_m, &hl_m, &img_m, &opts);

    for (docx_val, expected) in [
        (DocxVAlign::Top, CellVerticalAlign::Top),
        (DocxVAlign::Center, CellVerticalAlign::Middle),
        (DocxVAlign::Bottom, CellVerticalAlign::Bottom),
    ] {
        let tc = cell_with_props(DocxTcPr {
            v_align: Some(docx_val),
            ..Default::default()
        });
        let cell = map_cell(&tc, &mut ctx);
        assert_eq!(cell.props.vertical_align, Some(expected));
    }
}

#[test]
fn cell_text_direction_maps_correctly() {
    use crate::docx::model::styles::DocxTextDirection;
    use loki_doc_model::content::table::row::CellTextDirection;

    let styles = StyleCatalog::default();
    let (fn_m, en_m, hl_m, img_m) = (
        HashMap::new(),
        HashMap::new(),
        HashMap::new(),
        HashMap::new(),
    );
    let opts = DocxImportOptions::default();
    let mut ctx = make_ctx(&styles, &fn_m, &en_m, &hl_m, &img_m, &opts);

    for (docx_val, expected) in [
        (DocxTextDirection::LrTb, CellTextDirection::LrTb),
        (DocxTextDirection::TbRl, CellTextDirection::TbRl),
        (DocxTextDirection::TbLr, CellTextDirection::TbLr),
        (DocxTextDirection::BtLr, CellTextDirection::BtLr),
    ] {
        let tc = cell_with_props(DocxTcPr {
            text_direction: Some(docx_val),
            ..Default::default()
        });
        let cell = map_cell(&tc, &mut ctx);
        assert_eq!(cell.props.text_direction, Some(expected));
    }
}
