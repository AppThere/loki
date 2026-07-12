// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 AppThere Loki contributors

//! DOCX table serialization: `w:tbl`, its rows (`w:tr`, with vertical-merge
//! tracking) and cells (`w:tc`, whose block content recurses through
//! `super::write_blocks`). Split out of `write/document.rs` (Phase 7.1).

use quick_xml::Writer;

use loki_doc_model::content::table::core::Table;
use loki_doc_model::content::table::row::Cell;

use crate::docx::write::collector::ExportCollector;
use crate::docx::write::table_style::write_tbl_look;
use crate::docx::write::xml::{
    color_to_hex, pts_to_twips, write_empty, write_end, write_start, wval,
};

use super::write_blocks;

pub(super) fn write_table<W: std::io::Write>(
    w: &mut Writer<W>,
    tbl: &Table,
    collector: &mut ExportCollector,
) {
    let _ = write_start(w, "w:tbl", &[]);

    // Table properties: named style (if any, before tblW per the schema), width.
    let _ = write_start(w, "w:tblPr", &[]);
    if let Some(style) = tbl.style_name() {
        let _ = write_empty(w, "w:tblStyle", &[("w:val", style)]);
    }
    let _ = write_empty(w, "w:tblW", &[("w:w", "0"), ("w:type", "auto")]);
    write_tbl_look(w, tbl.table_look_code());
    let _ = write_end(w, "w:tblPr");

    // Grid columns.
    let col_count = tbl.col_specs.len();
    let mut row_span_tracker = vec![0u32; col_count];
    let _ = write_start(w, "w:tblGrid", &[]);
    for col in &tbl.col_specs {
        use loki_doc_model::content::table::col::ColWidth;
        let w_twips = match col.width {
            ColWidth::Fixed(pt) => pts_to_twips(pt.value()).to_string(),
            _ => "1440".to_string(),
        };
        let _ = write_empty(w, "w:gridCol", &[("w:w", &w_twips)]);
    }
    let _ = write_end(w, "w:tblGrid");

    // Header rows.
    for row in &tbl.head.rows {
        write_table_row(w, row, true, &mut row_span_tracker, collector);
    }
    // Body rows.
    for body in &tbl.bodies {
        for row in &body.head_rows {
            write_table_row(w, row, true, &mut row_span_tracker, collector);
        }
        for row in &body.body_rows {
            write_table_row(w, row, false, &mut row_span_tracker, collector);
        }
    }
    // Foot rows.
    for row in &tbl.foot.rows {
        write_table_row(w, row, false, &mut row_span_tracker, collector);
    }

    let _ = write_end(w, "w:tbl");
}

fn write_table_row<W: std::io::Write>(
    w: &mut Writer<W>,
    row: &loki_doc_model::content::table::row::Row,
    is_header: bool,
    row_span_tracker: &mut [u32],
    collector: &mut ExportCollector,
) {
    let _ = write_start(w, "w:tr", &[]);
    if is_header {
        let _ = write_start(w, "w:trPr", &[]);
        let _ = write_empty(w, "w:tblHeader", &[]);
        let _ = write_end(w, "w:trPr");
    }

    let mut col_idx = 0;
    let mut cell_it = row.cells.iter();

    while col_idx < row_span_tracker.len() {
        if row_span_tracker[col_idx] > 0 {
            // This column is covered by a merge from above.
            let _ = write_start(w, "w:tc", &[]);
            let _ = write_start(w, "w:tcPr", &[]);
            let _ = write_empty(w, "w:vMerge", &[]);
            let _ = write_end(w, "w:tcPr");
            let _ = write_start(w, "w:p", &[]);
            let _ = write_end(w, "w:p");
            let _ = write_end(w, "w:tc");

            row_span_tracker[col_idx] -= 1;
            col_idx += 1;
        } else if let Some(cell) = cell_it.next() {
            write_table_cell(w, cell, collector);

            if cell.row_span > 1 {
                for i in 0..cell.col_span as usize {
                    if col_idx + i < row_span_tracker.len() {
                        row_span_tracker[col_idx + i] = cell.row_span - 1;
                    }
                }
            }
            col_idx += cell.col_span as usize;
        } else {
            break; // no matching column in a valid model
        }
    }

    let _ = write_end(w, "w:tr");
}

fn write_table_cell<W: std::io::Write>(
    w: &mut Writer<W>,
    cell: &Cell,
    collector: &mut ExportCollector,
) {
    let _ = write_start(w, "w:tc", &[]);

    // Cell properties.
    let _ = write_start(w, "w:tcPr", &[]);
    if cell.col_span > 1 {
        let span_s = cell.col_span.to_string();
        let _ = write_empty(w, "w:gridSpan", &wval(&span_s));
    }
    if cell.row_span > 1 {
        let _ = write_empty(w, "w:vMerge", &wval("restart"));
    }
    let props = &cell.props;
    // Direct cell borders (`w:tcBorders`, 4a.3) — CT_TcPr order puts them
    // before shading and margins. (The reader has parsed these all along;
    // export silently dropped them until this pass.)
    let has_borders = props.border_top.is_some()
        || props.border_bottom.is_some()
        || props.border_left.is_some()
        || props.border_right.is_some();
    if has_borders {
        let _ = write_start(w, "w:tcBorders", &[]);
        // CT_TcBorders sequence: top, left, bottom, right.
        write_tc_border_edge(w, "w:top", props.border_top.as_ref());
        write_tc_border_edge(w, "w:left", props.border_left.as_ref());
        write_tc_border_edge(w, "w:bottom", props.border_bottom.as_ref());
        write_tc_border_edge(w, "w:right", props.border_right.as_ref());
        let _ = write_end(w, "w:tcBorders");
    }
    // Background color (shading) — before w:tcMar per CT_TcPr.
    if let Some(color) = &props.background_color {
        let hex = color_to_hex(color);
        let _ = write_empty(
            w,
            "w:shd",
            &[("w:val", "clear"), ("w:color", "auto"), ("w:fill", &hex)],
        );
    }
    // Padding (margins).
    let has_padding = props.padding_top.is_some()
        || props.padding_bottom.is_some()
        || props.padding_left.is_some()
        || props.padding_right.is_some();
    if has_padding {
        let _ = write_start(w, "w:tcMar", &[]);
        if let Some(pt) = props.padding_top {
            let v = pts_to_twips(pt.value()).to_string();
            let _ = write_empty(w, "w:top", &[("w:w", &v), ("w:type", "dxa")]);
        }
        if let Some(pt) = props.padding_bottom {
            let v = pts_to_twips(pt.value()).to_string();
            let _ = write_empty(w, "w:bottom", &[("w:w", &v), ("w:type", "dxa")]);
        }
        if let Some(pt) = props.padding_left {
            let v = pts_to_twips(pt.value()).to_string();
            let _ = write_empty(w, "w:left", &[("w:w", &v), ("w:type", "dxa")]);
        }
        if let Some(pt) = props.padding_right {
            let v = pts_to_twips(pt.value()).to_string();
            let _ = write_empty(w, "w:right", &[("w:w", &v), ("w:type", "dxa")]);
        }
        let _ = write_end(w, "w:tcMar");
    }
    // Vertical alignment — last of the emitted CT_TcPr children.
    if let Some(va) = props.vertical_align {
        use loki_doc_model::content::table::row::CellVerticalAlign;
        let v = match va {
            CellVerticalAlign::Middle => "center",
            CellVerticalAlign::Bottom => "bottom",
            _ => "top",
        };
        let _ = write_empty(w, "w:vAlign", &wval(v));
    }
    let _ = write_end(w, "w:tcPr");

    // Cell content — must have at least one paragraph.
    if cell.blocks.is_empty() {
        let _ = write_start(w, "w:p", &[]);
        let _ = write_end(w, "w:p");
    } else {
        write_blocks(w, &cell.blocks, collector, 0);
    }

    let _ = write_end(w, "w:tc");
}

/// Writes one `w:tcBorders` edge from a model [`Border`]: style → `w:val`,
/// width in points → `w:sz` (eighth-points), colour → hex (or `auto`).
/// `BorderStyle::None` writes `w:val="nil"` (an explicit no-border, distinct
/// from an absent edge, which inherits the table style).
fn write_tc_border_edge<W: std::io::Write>(
    w: &mut Writer<W>,
    tag: &str,
    border: Option<&loki_doc_model::style::props::border::Border>,
) {
    use loki_doc_model::style::props::border::BorderStyle;
    let Some(b) = border else { return };
    if b.style == BorderStyle::None {
        let _ = write_empty(w, tag, &wval("nil"));
        return;
    }
    let val = match b.style {
        BorderStyle::Dashed => "dashed",
        BorderStyle::Dotted => "dotted",
        BorderStyle::Double => "double",
        BorderStyle::Inset => "inset",
        BorderStyle::Outset => "outset",
        BorderStyle::Wave => "wave",
        // Groove/Ridge have no OOXML equivalent (threeDEmboss/threeDEngrave
        // are visually different); Solid and future variants map to single.
        _ => "single",
    };
    // Eighth-points, clamped to OOXML's valid 2..=96 w:sz range.
    #[allow(clippy::cast_possible_truncation)] // clamped to 2..=96 above the cast
    let sz = ((b.width.value() * 8.0).round().clamp(2.0, 96.0) as i32).to_string();
    let color = b
        .color
        .as_ref()
        .map_or_else(|| "auto".to_string(), color_to_hex);
    let _ = write_empty(
        w,
        tag,
        &[
            ("w:val", val),
            ("w:sz", &sz),
            ("w:space", "0"),
            ("w:color", &color),
        ],
    );
}
