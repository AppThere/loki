// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 AppThere Loki contributors

//! Worksheet XML parsing — extracts cell data.

use super::util::cell_ref_to_coord;
use crate::error::OoxmlError;
use crate::xml_util::local_name;
use loki_sheet_model::{Cell, CellStyle, Worksheet};
use quick_xml::Reader;
use quick_xml::events::Event;

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
                    use crate::xml_util::local_attr_val;
                    current_ref = local_attr_val(e, b"r");
                    current_type = local_attr_val(e, b"t");
                    current_style_idx = local_attr_val(e, b"s").and_then(|s| s.parse::<usize>().ok());
                    current_formula = None;
                    current_value.clear();
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
            Ok(Event::Text(ref e)) => {
                let text = e.unescape().unwrap_or_default().into_owned();
                if in_f {
                    current_formula = Some(text);
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
