// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 AppThere Loki contributors

//! Worksheet (`xl/worksheets/sheetN.xml`) parsing for the XLSX importer
//! (split from `import.rs` for the 300-line ceiling): reads cells (values via
//! the shared-strings table, formulas, per-cell style index) and column
//! widths into a `Worksheet`. Column-width conversion stays in `import.rs`;
//! the A1 cell-ref decoder lives here (its only caller).

use quick_xml::Reader;
use quick_xml::events::Event;

use super::xlsx_char_width_to_pt;
use crate::error::OoxmlError;
use crate::xml_util::{event_text, local_attr_val, local_attr_vals, local_name};
use loki_sheet_model::{Cell, CellStyle, Worksheet};

pub(super) fn parse_worksheet(
    data: &[u8],
    shared_strings: &[String],
    styles: &[CellStyle],
) -> Result<Worksheet, OoxmlError> {
    let mut worksheet = Worksheet::new("Sheet");
    let mut reader = Reader::from_reader(data);
    reader.config_mut().trim_text(false);
    let mut buf = Vec::new();

    let mut current_ref = None;
    let mut current_type = None;
    let mut current_style_idx = None;
    let mut current_formula = None;
    let mut current_value = String::new();

    let mut in_f = false;
    let mut in_v = false;
    let mut in_is_t = false;

    macro_rules! handle_start {
        ($e:expr) => {{
            let e = $e;
            match local_name(e) {
                b"c" => {
                    // One scan of the attribute list per cell instead of three.
                    let [r, t, s] = local_attr_vals(e, [b"r", b"t", b"s"]);
                    current_ref = r;
                    current_type = t;
                    current_style_idx = s.and_then(|s| s.parse::<usize>().ok());
                    current_formula = None;
                    current_value.clear();
                }
                b"col" => {
                    // <col min="1" max="3" width="12.5" customWidth="1"/> — widths
                    // are in character units; 1-based, inclusive range.
                    let min = local_attr_val(e, b"min").and_then(|s| s.parse::<u32>().ok());
                    let max = local_attr_val(e, b"max").and_then(|s| s.parse::<u32>().ok());
                    let width = local_attr_val(e, b"width").and_then(|s| s.parse::<f64>().ok());
                    if let (Some(min), Some(max), Some(width)) = (min, max, width) {
                        let pt = xlsx_char_width_to_pt(width);
                        let lo = min.saturating_sub(1);
                        // Cap the span so a "to the last column" range can't bloat the map.
                        let hi = max.saturating_sub(1).min(lo.saturating_add(1023));
                        for c in lo..=hi {
                            worksheet.set_column_width(c, pt);
                        }
                    }
                }
                b"f" => {
                    in_f = true;
                }
                b"v" => {
                    in_v = true;
                }
                b"t" => {
                    in_is_t = true;
                }
                _ => {}
            }
        }};
    }

    macro_rules! handle_end {
        ($name:expr) => {{
            match $name {
                b"c" => {
                    if let Some(r_str) = &current_ref {
                        if let Some((row, col)) = cell_ref_to_coord(r_str) {
                            let final_value = if current_type.as_deref() == Some("s") {
                                if let Ok(idx) = current_value.parse::<usize>() {
                                    shared_strings.get(idx).cloned().unwrap_or_default()
                                } else {
                                    current_value.clone()
                                }
                            } else {
                                current_value.clone()
                            };

                            let style = current_style_idx.and_then(|idx| styles.get(idx).cloned());

                            worksheet.cells.insert(
                                (row, col),
                                Cell {
                                    value: final_value,
                                    formula: current_formula.clone(),
                                    style,
                                },
                            );
                        }
                    }
                    current_ref = None;
                    current_type = None;
                    current_style_idx = None;
                }
                b"f" => {
                    in_f = false;
                }
                b"v" => {
                    in_v = false;
                }
                b"t" => {
                    in_is_t = false;
                }
                _ => {}
            }
        }};
    }

    loop {
        match reader.read_event_into(&mut buf) {
            Ok(Event::Start(ref e)) => handle_start!(e),
            Ok(Event::End(ref e)) => {
                let name_bytes = e.local_name().into_inner();
                let name = if let Some(pos) = name_bytes.iter().position(|&b| b == b':') {
                    &name_bytes[pos + 1..]
                } else {
                    name_bytes
                };
                handle_end!(name);
            }
            Ok(Event::Empty(ref e)) => {
                handle_start!(e);
                let name_bytes = e.local_name().into_inner();
                let name = if let Some(pos) = name_bytes.iter().position(|&b| b == b':') {
                    &name_bytes[pos + 1..]
                } else {
                    name_bytes
                };
                handle_end!(name);
            }
            Ok(ref ev @ (Event::Text(_) | Event::GeneralRef(_))) => {
                let text = event_text(ev).unwrap_or_default();
                if in_f {
                    // Accumulate: a formula containing an XML-escaped char
                    // (`&gt;`, `&lt;`, `&amp;`, `&quot;` — common in logical /
                    // comparison formulas) arrives as several Text/GeneralRef
                    // events. Overwriting would keep only the final fragment.
                    current_formula
                        .get_or_insert_with(String::new)
                        .push_str(&text);
                } else if in_v || in_is_t {
                    current_value.push_str(&text);
                }
            }
            Ok(Event::Eof) => break,
            Err(source) => {
                return Err(OoxmlError::Xml {
                    part: "sheet.xml".to_owned(),
                    source,
                });
            }
            _ => {}
        }
        buf.clear();
    }

    Ok(worksheet)
}

/// Decodes an A1-style cell reference (`"AB12"`) into zero-based `(row, col)`.
fn cell_ref_to_coord(cell_ref: &str) -> Option<(u32, u32)> {
    // Allocation-free split of "AB12" into column letters and row digits —
    // this runs once per cell on import. The leading letters are single-byte
    // ASCII, so `split` always lands on a char boundary; a non-digit tail
    // (or a non-ASCII byte) simply fails the row parse, as before.
    let bytes = cell_ref.as_bytes();
    let split = bytes
        .iter()
        .position(|b| !b.is_ascii_alphabetic())
        .unwrap_or(bytes.len());
    if split == 0 || split == bytes.len() {
        return None;
    }
    let mut col: u32 = 0;
    for &b in &bytes[..split] {
        col = col
            .checked_mul(26)?
            .checked_add(u32::from(b.to_ascii_uppercase() - b'A') + 1)?;
    }
    let col = col.checked_sub(1)?;
    let row = cell_ref[split..].parse::<u32>().ok()?.checked_sub(1)?;
    Some((row, col))
}
