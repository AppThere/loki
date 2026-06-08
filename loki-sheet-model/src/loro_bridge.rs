// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 AppThere Loki contributors

use crate::workbook::{
    Cell, CellAlign, CellStyle, DocumentMeta, NumberFormat, Workbook, Worksheet,
};
use loro::{LoroDoc, LoroMap};
use std::collections::HashMap;

pub const KEY_METADATA: &str = "metadata";
pub const KEY_SHEETS: &str = "sheets";

#[derive(Debug, thiserror::Error)]
pub enum BridgeError {
    #[error("Loro error: {0}")]
    Loro(String),
}

impl From<loro::LoroError> for BridgeError {
    fn from(e: loro::LoroError) -> Self {
        BridgeError::Loro(e.to_string())
    }
}

// Helper accessors for Loro maps
fn get_str_from_map(map: &LoroMap, key: &str) -> Option<String> {
    map.get(key)
        .and_then(|v| v.into_value().ok())
        .and_then(|v| v.into_string().ok())
        .map(|s| s.to_string())
}

fn get_bool_from_map(map: &LoroMap, key: &str) -> Option<bool> {
    map.get(key)
        .and_then(|v| v.into_value().ok())
        .and_then(|v| v.into_bool().ok())
}

/// Converts a Loki workbook model into a fresh LoroDoc.
pub fn workbook_to_loro(wb: &Workbook) -> Result<LoroDoc, BridgeError> {
    let loro_doc = LoroDoc::new();

    // Metadata
    let meta_map = loro_doc.get_map(KEY_METADATA);
    if let Some(title) = &wb.meta.title {
        meta_map.insert("title", title.as_str())?;
    }
    if let Some(creator) = &wb.meta.creator {
        meta_map.insert("creator", creator.as_str())?;
    }

    // Sheets list
    let sheets_list = loro_doc.get_list(KEY_SHEETS);
    for (s_idx, sheet) in wb.sheets.iter().enumerate() {
        let sheet_map = sheets_list.insert_container(s_idx, LoroMap::new())?;
        sheet_map.insert("name", sheet.name.as_str())?;

        let cells_map = sheet_map.insert_container("cells", LoroMap::new())?;
        for (&(row, col), cell) in &sheet.cells {
            let key = format!("{},{}", row, col);
            let cell_map = cells_map.insert_container(&key, LoroMap::new())?;
            cell_map.insert("value", cell.value.as_str())?;
            if let Some(formula) = &cell.formula {
                cell_map.insert("formula", formula.as_str())?;
            }
            if let Some(style) = &cell.style {
                let style_map = cell_map.insert_container("style", LoroMap::new())?;
                style_map.insert("bold", style.bold)?;
                style_map.insert("italic", style.italic)?;
                style_map.insert("underline", style.underline)?;
                style_map.insert("align", style.align.as_str())?;
                style_map.insert("num_format", style.num_format.as_str())?;
            }
        }
    }

    Ok(loro_doc)
}

/// Converts a LoroDoc back into a Workbook snapshot.
pub fn loro_to_workbook(loro: &LoroDoc) -> Result<Workbook, BridgeError> {
    let mut wb = Workbook {
        meta: DocumentMeta::default(),
        sheets: Vec::new(),
    };

    // Metadata
    let meta_map = loro.get_map(KEY_METADATA);
    wb.meta.title = get_str_from_map(&meta_map, "title");
    wb.meta.creator = get_str_from_map(&meta_map, "creator");

    // Sheets list
    let sheets_list = loro.get_list(KEY_SHEETS);
    for i in 0..sheets_list.len() {
        let Some(sheet_val) = sheets_list.get(i) else {
            continue;
        };
        let Some(sheet_map) = sheet_val
            .into_container()
            .ok()
            .and_then(|c| c.into_map().ok())
        else {
            continue;
        };

        let name =
            get_str_from_map(&sheet_map, "name").unwrap_or_else(|| format!("Sheet{}", i + 1));
        let mut cells = HashMap::new();

        if let Some(cells_map) = sheet_map
            .get("cells")
            .and_then(|val| val.into_container().ok())
            .and_then(|c| c.into_map().ok())
        {
            for key_string in cells_map.keys() {
                let key_str: &str = key_string.as_ref();
                let parts: Vec<&str> = key_str.split(',').collect();
                if let Some((cell_map, row, col)) = (parts.len() == 2)
                    .then(|| {
                        let row = parts[0].parse::<u32>().ok()?;
                        let col = parts[1].parse::<u32>().ok()?;
                        let cell_map = cells_map
                            .get(key_str)?
                            .into_container()
                            .ok()?
                            .into_map()
                            .ok()?;
                        Some((cell_map, row, col))
                    })
                    .flatten()
                {
                    let value = get_str_from_map(&cell_map, "value").unwrap_or_default();
                    let formula = get_str_from_map(&cell_map, "formula");
                    let mut style = None;

                    if let Some(style_map) = cell_map
                        .get("style")
                        .and_then(|val| val.into_container().ok())
                        .and_then(|c| c.into_map().ok())
                    {
                        let bold = get_bool_from_map(&style_map, "bold").unwrap_or(false);
                        let italic = get_bool_from_map(&style_map, "italic").unwrap_or(false);
                        let underline = get_bool_from_map(&style_map, "underline").unwrap_or(false);
                        let align = get_str_from_map(&style_map, "align")
                            .map(|s| match s.as_str() {
                                "center" => CellAlign::Center,
                                "right" => CellAlign::Right,
                                _ => CellAlign::Left,
                            })
                            .unwrap_or(CellAlign::Left);
                        let num_format = get_str_from_map(&style_map, "num_format")
                            .map(|s| match s.as_str() {
                                "currency" => NumberFormat::Currency,
                                "percent" => NumberFormat::Percent,
                                _ => NumberFormat::General,
                            })
                            .unwrap_or(NumberFormat::General);

                        style = Some(CellStyle {
                            bold,
                            italic,
                            underline,
                            align,
                            num_format,
                        });
                    }

                    cells.insert(
                        (row, col),
                        Cell {
                            value,
                            formula,
                            style,
                        },
                    );
                }
            }
        }

        wb.sheets.push(Worksheet { name, cells });
    }

    if wb.sheets.is_empty() {
        wb.sheets.push(Worksheet::new("Sheet1"));
    }

    Ok(wb)
}
