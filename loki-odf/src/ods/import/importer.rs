// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 AppThere Loki contributors

//! `OdsImport` implementation — reads an ODS package and produces a `Workbook`.

use std::io::{Read, Seek};

use loki_sheet_model::{Cell, DocumentMeta, Workbook, Worksheet};
use quick_xml::Reader;
use quick_xml::events::Event;

use crate::constants::ENTRY_CONTENT;
use crate::error::OdfError;
use crate::package::OdfPackage;
use crate::xml_util::local_attr_val;

use super::styles::parse_ods_styles;
use super::xml_helpers::{clean_ods_formula, local_name, local_name_end};

/// Options controlling ODS import behaviour.
#[derive(Debug, Clone, Default)]
pub struct OdsImportOptions {}

/// The result of a successful ODS import.
#[derive(Debug)]
pub struct OdsImportResult {
    /// The imported workbook model.
    pub workbook: Workbook,
}

/// Unit struct that implements ODS spreadsheet import.
pub struct OdsImport;

impl OdsImport {
    /// Imports an ODS file and returns the workbook.
    pub fn import(
        reader: impl Read + Seek,
        _options: OdsImportOptions,
    ) -> Result<Workbook, OdfError> {
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
        let mut row_idx = 0;
        let mut col_idx = 0;

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
                                current_row_repeated =
                                    local_attr_val(e, b"number-rows-repeated")
                                        .and_then(|s| s.parse::<u32>().ok())
                                        .unwrap_or(1);
                                in_row = true;
                            }
                        }
                        b"table-cell" => {
                            if in_row {
                                cell_formula = local_attr_val(e, b"formula");
                                cell_style_name = local_attr_val(e, b"style-name");
                                cell_cols_repeated =
                                    local_attr_val(e, b"number-columns-repeated")
                                        .and_then(|s| s.parse::<u32>().ok())
                                        .unwrap_or(1);

                                office_value = local_attr_val(e, b"value");
                                office_string_value = local_attr_val(e, b"string-value");
                                office_boolean_value = local_attr_val(e, b"boolean-value");
                                office_date_value = local_attr_val(e, b"date-value");

                                p_text.clear();
                                in_cell = true;
                            }
                        }
                        b"p" => {
                            if in_cell {
                                in_p = true;
                            }
                        }
                        _ => {}
                    }
                }
                Ok(Event::Empty(ref e)) => {
                    let local = local_name(e);
                    match local {
                        b"table-cell" => {
                            if in_row {
                                let style_name = local_attr_val(e, b"style-name");
                                let cols_repeated =
                                    local_attr_val(e, b"number-columns-repeated")
                                        .and_then(|s| s.parse::<u32>().ok())
                                        .unwrap_or(1);

                                let style =
                                    style_name.and_then(|name| styles.get(&name).cloned());
                                if let Some(ref mut ws) = current_sheet {
                                    if style.is_some() {
                                        for r in row_idx..(row_idx + current_row_repeated) {
                                            for c in col_idx..(col_idx + cols_repeated) {
                                                ws.cells.insert(
                                                    (r, c),
                                                    Cell {
                                                        value: String::new(),
                                                        formula: None,
                                                        style: style.clone(),
                                                    },
                                                );
                                            }
                                        }
                                    }
                                }
                                col_idx += cols_repeated;
                            }
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
                                        for r in row_idx..(row_idx + current_row_repeated) {
                                            for c in col_idx..(col_idx + cell_cols_repeated) {
                                                ws.cells.insert(
                                                    (r, c),
                                                    Cell {
                                                        value: raw_val.clone(),
                                                        formula: cleaned_formula.clone(),
                                                        style: style.clone(),
                                                    },
                                                );
                                            }
                                        }
                                    }
                                }

                                col_idx += cell_cols_repeated;
                                in_cell = false;
                            }
                        }
                        b"table-row" => {
                            if in_row {
                                row_idx += current_row_repeated;
                                in_row = false;
                            }
                        }
                        b"table" => {
                            if in_table {
                                if let Some(ws) = current_sheet.take() {
                                    sheets.push(ws);
                                }
                                in_table = false;
                            }
                        }
                        _ => {}
                    }
                }
                Ok(Event::Text(ref e)) => {
                    if in_p {
                        if let Ok(text) = e.unescape() {
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

        Ok(Workbook {
            meta: DocumentMeta::default(),
            sheets,
        })
    }
}
