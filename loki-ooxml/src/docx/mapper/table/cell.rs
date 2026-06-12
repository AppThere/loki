// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 AppThere Loki contributors

//! Cell mapper: `w:tc` → `Cell`.

use loki_doc_model::content::attr::NodeAttr;
use loki_doc_model::content::block::Block;
use loki_doc_model::content::table::col::ColAlignment;
use loki_doc_model::content::table::row::{Cell, CellProps, CellTextDirection, CellVerticalAlign};
use loki_primitives::units::Points;

use crate::docx::model::styles::{DocxTableCell, DocxTextDirection, DocxVAlign};

use super::super::document::MappingContext;
use super::super::paragraph::map_paragraph;
use super::super::props::map_border_edge;

/// Maps a `w:tc` table cell.
pub(crate) fn map_cell(tc: &DocxTableCell, ctx: &mut MappingContext<'_>) -> Cell {
    let col_span = tc.tc_pr.as_ref().and_then(|p| p.grid_span).unwrap_or(1);
    let blocks: Vec<Block> = tc
        .paragraphs
        .iter()
        .flat_map(|p| map_paragraph(p, ctx))
        .collect();

    let mut props = CellProps::default();
    if let Some(tc_pr) = tc.tc_pr.as_ref() {
        // Cell background from `w:shd @w:fill`.
        if let Some(ref hex) = tc_pr.shd_fill
            && let Some(rgb) = crate::xml_util::hex_color(hex)
        {
            use loki_primitives::color::DocumentColor;
            props.background_color = Some(DocumentColor::Rgb(rgb));
        }
        // Cell borders from `w:tcBorders`.
        if let Some(ref borders) = tc_pr.tc_borders {
            props.border_top = borders.top.as_ref().map(map_border_edge);
            props.border_bottom = borders.bottom.as_ref().map(map_border_edge);
            props.border_left = borders.left.as_ref().map(map_border_edge);
            props.border_right = borders.right.as_ref().map(map_border_edge);
        }
        // Cell padding from `w:tcMar`. COMPAT(ooxml-dxa): twips ÷ 20 = points.
        if let Some(ref m) = tc_pr.tc_margins {
            props.padding_top = m.top.map(|v| Points::new(f64::from(v) / 20.0));
            props.padding_bottom = m.bottom.map(|v| Points::new(f64::from(v) / 20.0));
            props.padding_left = m.left.map(|v| Points::new(f64::from(v) / 20.0));
            props.padding_right = m.right.map(|v| Points::new(f64::from(v) / 20.0));
        }
        // Vertical alignment from `w:vAlign`.
        props.vertical_align = tc_pr.v_align.map(|v| match v {
            DocxVAlign::Top => CellVerticalAlign::Top,
            DocxVAlign::Center => CellVerticalAlign::Middle,
            DocxVAlign::Bottom => CellVerticalAlign::Bottom,
        });
        // Text direction from `w:textDirection`.
        props.text_direction = tc_pr.text_direction.map(|d| match d {
            DocxTextDirection::LrTb => CellTextDirection::LrTb,
            DocxTextDirection::TbRl => CellTextDirection::TbRl,
            DocxTextDirection::TbLr => CellTextDirection::TbLr,
            DocxTextDirection::BtLr => CellTextDirection::BtLr,
        });
    }

    Cell {
        attr: NodeAttr::default(),
        alignment: ColAlignment::Default,
        row_span: 1, // overridden by map_table after compute_v_merge_spans
        col_span,
        blocks,
        props,
    }
}
