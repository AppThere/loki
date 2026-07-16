// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 AppThere Loki contributors

//! ODS importer.

use loki_doc_model::io::macros::MacroPayload;
use loki_sheet_model::{Cell, DocumentMeta, Workbook, Worksheet};
use quick_xml::Reader;
use quick_xml::events::Event;
use std::io::{Read, Seek};

use crate::constants::ENTRY_CONTENT;
use crate::error::OdfError;
use crate::limits::{MAX_MATERIALIZED_REPEAT, MAX_SHEET_COLS, MAX_SHEET_ROWS};
use crate::package::OdfPackage;
use crate::xml_util::{event_text, local_attr_val};

#[path = "import_helpers.rs"]
mod helpers;

use helpers::{clean_ods_formula, fill_cells, local_name, local_name_end, parse_ods_styles};

/// Options controlling ODS import behaviour.
#[derive(Debug, Clone, Default)]
pub struct OdsImportOptions {}

/// The result of a successful ODS import.
#[derive(Debug)]
pub struct OdsImportResult {
    /// The imported workbook model.
    pub workbook: Workbook,
    /// Preserved `StarBasic` / script payload, if present. Not executed in
    /// Phase 1; retained so a macro-carrying re-export does not strip it
    /// (spec §3).
    pub macros: Option<MacroPayload>,
}

/// Unit struct that implements ODS spreadsheet import.
pub struct OdsImport;

impl OdsImport {
    /// Imports an ODS file and returns the workbook.
    ///
    /// Discards any preserved macro payload; use [`OdsImport::run`] to keep it.
    pub fn import(
        reader: impl Read + Seek,
        options: OdsImportOptions,
    ) -> Result<Workbook, OdfError> {
        Self::run(reader, options).map(|r| r.workbook)
    }

    /// Imports an ODS file, returning the workbook plus any preserved macro
    /// payload.
    pub fn run(
        reader: impl Read + Seek,
        _options: OdsImportOptions,
    ) -> Result<OdsImportResult, OdfError> {
        let package = OdfPackage::open(reader)?;

        // 1. Parse ODS styles
        let styles = parse_ods_styles(&package.content, &package.styles);

        // 2. Parse worksheets
        let mut reader = Reader::from_reader(package.content.as_slice());
        reader.config_mut().trim_text(false);
        let mut buf = Vec::new();

        let mut sheets = Vec::new();
        let mut current_sheet = None;
        let mut current_row_repeated = 1;
        let mut row_idx: u32 = 0;
        let mut col_idx: u32 = 0;
        // Aggregate count of materialized cells across the whole workbook,
        // bounding the row×column amplification the per-axis caps allow.
        let mut materialized_cells: u64 = 0;

        let mut in_table = false;
        let mut in_row = false;
        let mut in_cell = false;
        let mut in_p = false;

        let mut cell_formula = None;
        let mut cell_style_name = None;
        let mut cell_cols_repeated = 1;

        let mut office_value = None;
        let mut office_string_value = None;
        let mut office_boolean_value = None;
        let mut office_date_value = None;

        let mut p_text = String::new();

        loop {
            match reader.read_event_into(&mut buf) {
                Ok(Event::Start(ref e)) => {
                    let local = local_name(e);
                    match local {
                        b"table" => {
                            if let Some(name) = local_attr_val(e, b"name") {
                                current_sheet = Some(Worksheet::new(name));
                                row_idx = 0;
                                in_table = true;
                            }
                        }
                        b"table-row" => {
                            if in_table {
                                col_idx = 0;
                                // Attacker-controlled repeat count: clamp to the
                                // sheet's row space so index math stays bounded.
                                current_row_repeated = local_attr_val(e, b"number-rows-repeated")
                                    .and_then(|s| s.parse::<u32>().ok())
                                    .unwrap_or(1)
                                    .min(MAX_SHEET_ROWS);
                                in_row = true;
                            }
                        }
                        b"table-cell" => {
                            if in_row {
                                cell_formula = local_attr_val(e, b"formula");
                                cell_style_name = local_attr_val(e, b"style-name");
                                // Attacker-controlled repeat count: clamp to the
                                // sheet's column space so index math stays bounded.
                                cell_cols_repeated = local_attr_val(e, b"number-columns-repeated")
                                    .and_then(|s| s.parse::<u32>().ok())
                                    .unwrap_or(1)
                                    .min(MAX_SHEET_COLS);

                                office_value = local_attr_val(e, b"value");
                                office_string_value = local_attr_val(e, b"string-value");
                                office_boolean_value = local_attr_val(e, b"boolean-value");
                                office_date_value = local_attr_val(e, b"date-value");

                                p_text.clear();
                                in_cell = true;
                            }
                        }
                        b"p" if in_cell => {
                            in_p = true;
                        }
                        _ => {}
                    }
                }
                Ok(Event::Empty(ref e)) => {
                    let local = local_name(e);
                    match local {
                        b"table-cell" if in_row => {
                            let style_name = local_attr_val(e, b"style-name");
                            let cols_repeated = local_attr_val(e, b"number-columns-repeated")
                                .and_then(|s| s.parse::<u32>().ok())
                                .unwrap_or(1)
                                .min(MAX_SHEET_COLS);

                            let style = style_name.and_then(|name| styles.get(&name).cloned());
                            if let Some(ref mut ws) = current_sheet {
                                if style.is_some() {
                                    // Only materialize a bounded number of
                                    // cells; the cursor still advances by the
                                    // full clamped repeat count below.
                                    let mat_rows =
                                        current_row_repeated.min(MAX_MATERIALIZED_REPEAT);
                                    let mat_cols = cols_repeated.min(MAX_MATERIALIZED_REPEAT);
                                    fill_cells(
                                        ws,
                                        row_idx,
                                        col_idx,
                                        mat_rows,
                                        mat_cols,
                                        &mut materialized_cells,
                                        &Cell {
                                            value: String::new(),
                                            formula: None,
                                            style: style.clone(),
                                        },
                                    );
                                }
                            }
                            col_idx = col_idx.saturating_add(cols_repeated);
                        }
                        _ => {}
                    }
                }
                Ok(Event::End(ref e)) => {
                    let local = local_name_end(e);
                    match local {
                        b"p" => {
                            in_p = false;
                        }
                        b"table-cell" => {
                            if in_cell {
                                let raw_val = office_string_value
                                    .take()
                                    .or(office_value.take())
                                    .or(office_boolean_value.take())
                                    .or(office_date_value.take())
                                    .unwrap_or_else(|| p_text.clone());

                                let cleaned_formula =
                                    cell_formula.take().map(|f| clean_ods_formula(&f));
                                let style = cell_style_name
                                    .take()
                                    .and_then(|name| styles.get(&name).cloned());

                                if let Some(ref mut ws) = current_sheet {
                                    if !raw_val.is_empty()
                                        || cleaned_formula.is_some()
                                        || style.is_some()
                                    {
                                        // Only materialize a bounded number of
                                        // cells; the cursor still advances by the
                                        // full clamped repeat count below.
                                        let mat_rows =
                                            current_row_repeated.min(MAX_MATERIALIZED_REPEAT);
                                        let mat_cols =
                                            cell_cols_repeated.min(MAX_MATERIALIZED_REPEAT);
                                        fill_cells(
                                            ws,
                                            row_idx,
                                            col_idx,
                                            mat_rows,
                                            mat_cols,
                                            &mut materialized_cells,
                                            &Cell {
                                                value: raw_val.clone(),
                                                formula: cleaned_formula.clone(),
                                                style: style.clone(),
                                            },
                                        );
                                    }
                                }

                                col_idx = col_idx.saturating_add(cell_cols_repeated);
                                in_cell = false;
                            }
                        }
                        b"table-row" => {
                            if in_row {
                                row_idx = row_idx.saturating_add(current_row_repeated);
                                in_row = false;
                            }
                        }
                        b"table" if in_table => {
                            if let Some(ws) = current_sheet.take() {
                                sheets.push(ws);
                            }
                            in_table = false;
                        }
                        _ => {}
                    }
                }
                Ok(ref ev @ (Event::Text(_) | Event::GeneralRef(_))) => {
                    if in_p {
                        if let Ok(text) = event_text(ev) {
                            p_text.push_str(&text);
                        }
                    }
                }
                Ok(Event::Eof) => break,
                Err(source) => {
                    return Err(OdfError::Xml {
                        part: ENTRY_CONTENT.to_owned(),
                        source,
                    });
                }
                _ => {}
            }
            buf.clear();
        }

        if sheets.is_empty() {
            sheets.push(Worksheet::new("Sheet1"));
        }

        Ok(OdsImportResult {
            workbook: Workbook {
                meta: DocumentMeta::default(),
                sheets,
            },
            macros: package.macros,
        })
    }
}
