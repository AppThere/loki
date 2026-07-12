// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 AppThere Loki contributors

//! XML parsing helpers for the ODS importer (split from `import.rs` for the
//! 300-line ceiling): the cell-repeat materializer, the namespace-stripping
//! `local_name` accessors, the ODS→internal formula cleaner, and the data /
//! cell-style parser. All are re-imported by `import.rs`, which owns the main
//! `content.xml` streaming parse.

use std::collections::HashMap;

use quick_xml::Reader;
use quick_xml::events::Event;

use crate::limits::MAX_MATERIALIZED_CELLS_TOTAL;
use crate::xml_util::local_attr_val;
use loki_sheet_model::{Cell, CellAlign, CellStyle, NumberFormat, Worksheet};

// ── XML Parsing Helpers ──────────────────────────────────────────────────────

/// Inserts copies of `template` into the `mat_rows × mat_cols` block whose
/// top-left corner is `(row_idx, col_idx)`, charging each insert against the
/// workbook-wide [`MAX_MATERIALIZED_CELLS_TOTAL`] budget. Once the budget is
/// exhausted no further cells are inserted (the caller still advances its
/// cursor), bounding the row×column amplification the per-axis caps permit.
pub(super) fn fill_cells(
    ws: &mut Worksheet,
    row_idx: u32,
    col_idx: u32,
    mat_rows: u32,
    mat_cols: u32,
    materialized_cells: &mut u64,
    template: &Cell,
) {
    for r in row_idx..row_idx.saturating_add(mat_rows) {
        for c in col_idx..col_idx.saturating_add(mat_cols) {
            if *materialized_cells >= MAX_MATERIALIZED_CELLS_TOTAL {
                return;
            }
            ws.cells.insert((r, c), template.clone());
            *materialized_cells += 1;
        }
    }
}

pub(super) fn local_name<'a>(e: &'a quick_xml::events::BytesStart<'a>) -> &'a [u8] {
    let name = e.local_name().into_inner();
    if let Some(pos) = name.iter().position(|&b| b == b':') {
        &name[pos + 1..]
    } else {
        name
    }
}

pub(super) fn local_name_end<'a>(e: &'a quick_xml::events::BytesEnd<'a>) -> &'a [u8] {
    let name = e.local_name().into_inner();
    if let Some(pos) = name.iter().position(|&b| b == b':') {
        &name[pos + 1..]
    } else {
        name
    }
}

pub(super) fn clean_ods_formula(formula: &str) -> String {
    let s = formula.trim();
    let s = if let Some(stripped) = s.strip_prefix("of:=") {
        stripped
    } else if let Some(stripped) = s.strip_prefix("oooc:=") {
        stripped
    } else if let Some(stripped) = s.strip_prefix('=') {
        stripped
    } else {
        s
    };

    let mut result = String::new();
    let mut chars = s.chars().peekable();
    while let Some(c) = chars.next() {
        match c {
            '[' => {
                if chars.peek() == Some(&'.') {
                    chars.next();
                }
            }
            ']' => {}
            '.' => {
                if !result.ends_with(':') {
                    result.push(c);
                }
            }
            _ => {
                result.push(c);
            }
        }
    }
    format!("={result}")
}

pub(super) fn parse_ods_styles(
    content_xml: &[u8],
    styles_xml: &[u8],
) -> HashMap<String, CellStyle> {
    let mut data_styles = HashMap::new();

    // First Pass: Collect data styles
    let mut collect_data_styles = |data: &[u8]| {
        let mut reader = Reader::from_reader(data);
        reader.config_mut().trim_text(false);
        let mut buf = Vec::new();
        loop {
            match reader.read_event_into(&mut buf) {
                Ok(Event::Start(ref e)) | Ok(Event::Empty(ref e)) => {
                    let local = local_name(e);
                    match local {
                        b"number-style" | b"percentage-style" | b"currency-style"
                        | b"date-style" | b"time-style" => {
                            if let Some(name) = local_attr_val(e, b"name") {
                                let fmt = match local {
                                    b"percentage-style" => NumberFormat::Percent,
                                    b"currency-style" => NumberFormat::Currency,
                                    _ => NumberFormat::General,
                                };
                                data_styles.insert(name, fmt);
                            }
                        }
                        _ => {}
                    }
                }
                Ok(Event::Eof) => break,
                _ => {}
            }
            buf.clear();
        }
    };

    collect_data_styles(styles_xml);
    collect_data_styles(content_xml);

    // Second Pass: Parse cell styles
    let mut styles_map = HashMap::new();
    let mut parse_cell_styles = |data: &[u8]| {
        let mut reader = Reader::from_reader(data);
        reader.config_mut().trim_text(false);
        let mut buf = Vec::new();

        let mut current_style_name = None;
        let mut current_style = CellStyle::default();
        let mut current_data_style = None;
        let mut in_style = false;

        loop {
            match reader.read_event_into(&mut buf) {
                Ok(Event::Start(ref e)) | Ok(Event::Empty(ref e)) => {
                    let local = local_name(e);
                    if local == b"style" {
                        if let Some(family) = local_attr_val(e, b"family") {
                            if family == "table-cell" {
                                current_style_name = local_attr_val(e, b"name");
                                current_style = CellStyle::default();
                                current_data_style = local_attr_val(e, b"data-style-name");
                                in_style = true;
                            }
                        }
                    } else if in_style {
                        match local {
                            b"text-properties" => {
                                if let Some(weight) = local_attr_val(e, b"font-weight") {
                                    if weight == "bold" {
                                        current_style.bold = true;
                                    }
                                }
                                if let Some(italic) = local_attr_val(e, b"font-style") {
                                    if italic == "italic" || italic == "oblique" {
                                        current_style.italic = true;
                                    }
                                }
                                if let Some(underline) = local_attr_val(e, b"text-underline-style")
                                {
                                    if underline != "none" {
                                        current_style.underline = true;
                                    }
                                }
                            }
                            b"paragraph-properties" => {
                                if let Some(align_str) = local_attr_val(e, b"text-align") {
                                    current_style.align = match align_str.as_str() {
                                        "center" => CellAlign::Center,
                                        "right" | "end" => CellAlign::Right,
                                        _ => CellAlign::Left,
                                    };
                                }
                            }
                            _ => {}
                        }
                    }
                }
                Ok(Event::End(ref e)) => {
                    let local = local_name_end(e);
                    if local == b"style" && in_style {
                        if let Some(name) = current_style_name.take() {
                            if let Some(ref ds_name) = current_data_style {
                                if let Some(fmt) = data_styles.get(ds_name) {
                                    current_style.num_format = *fmt;
                                }
                            }
                            styles_map.insert(name, current_style.clone());
                        }
                        current_data_style = None;
                        in_style = false;
                    }
                }
                Ok(Event::Eof) => break,
                _ => {}
            }
            buf.clear();
        }
    };

    parse_cell_styles(styles_xml);
    parse_cell_styles(content_xml);

    styles_map
}
