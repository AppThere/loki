// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 AppThere Loki contributors

//! `xl/styles.xml` parsing for the XLSX importer (split from `import.rs` for
//! the 300-line ceiling): resolves fonts + `cellXfs` into the flat
//! `Vec<CellStyle>` the worksheet reader indexes by a cell's `s` attribute.

use std::collections::HashMap;

use quick_xml::Reader;
use quick_xml::events::Event;

use crate::error::OoxmlError;
use crate::xml_util::{local_attr_val, local_name};
use loki_sheet_model::{CellAlign, CellStyle, NumberFormat};

pub(super) fn parse_styles(data: &[u8]) -> Result<Vec<CellStyle>, OoxmlError> {
    let mut reader = Reader::from_reader(data);
    reader.config_mut().trim_text(false);
    let mut buf = Vec::new();

    let mut custom_num_formats = HashMap::new();
    let mut fonts = Vec::new();
    let mut cell_xfs = Vec::new();

    let mut in_cell_xfs = false;
    let mut in_font = false;
    let mut current_font = CellStyle::default();

    macro_rules! handle_start {
        ($e:expr) => {{
            let e = $e;
            match local_name(e) {
                b"numFmt" => {
                    if let (Some(id_str), Some(code)) = (local_attr_val(e, b"numFmtId"), local_attr_val(e, b"formatCode")) {
                        if let Ok(id) = id_str.parse::<u32>() {
                            let code_lower = code.to_lowercase();
                            let fmt = if code_lower.contains('%') {
                                NumberFormat::Percent
                            } else if code_lower.contains('$') || code_lower.contains('£') || code_lower.contains('€') || code_lower.contains('¥') {
                                NumberFormat::Currency
                            } else {
                                NumberFormat::General
                            };
                            custom_num_formats.insert(id, fmt);
                        }
                    }
                }
                b"font" => {
                    current_font = CellStyle::default();
                    in_font = true;
                }
                b"b" => {
                    if in_font {
                        current_font.bold = true;
                    }
                }
                b"i" => {
                    if in_font {
                        current_font.italic = true;
                    }
                }
                b"u" => {
                    if in_font {
                        current_font.underline = true;
                    }
                }
                b"cellXfs" => {
                    in_cell_xfs = true;
                }
                b"xf" => {
                    if in_cell_xfs {
                        let font_id = local_attr_val(e, b"fontId")
                            .and_then(|s| s.parse::<usize>().ok())
                            .unwrap_or(0);
                        let num_fmt_id = local_attr_val(e, b"numFmtId")
                            .and_then(|s| s.parse::<u32>().ok())
                            .unwrap_or(0);

                        let mut style = CellStyle::default();
                        style.bold = fonts.get(font_id).map(|f: &CellStyle| f.bold).unwrap_or(false);
                        style.italic = fonts.get(font_id).map(|f: &CellStyle| f.italic).unwrap_or(false);
                        style.underline = fonts.get(font_id).map(|f: &CellStyle| f.underline).unwrap_or(false);

                        let num_fmt = match num_fmt_id {
                            9 | 10 => NumberFormat::Percent,
                            5 | 6 | 7 | 8 | 44 => NumberFormat::Currency,
                            id => custom_num_formats.get(&id).cloned().unwrap_or(NumberFormat::General),
                        };
                        style.num_format = num_fmt;

                        cell_xfs.push(style);
                    }
                }
                b"alignment" if in_cell_xfs => {
                    if let Some(last_xf) = cell_xfs.last_mut()
                        && let Some(horiz) = local_attr_val(e, b"horizontal")
                    {
                        last_xf.align = match horiz.as_str() {
                            "center" => CellAlign::Center,
                            "right" => CellAlign::Right,
                            _ => CellAlign::Left,
                        };
                    }
                }
                _ => {}
            }
        }};
    }

    macro_rules! handle_end {
        ($name:expr) => {{
            match $name {
                b"font" => {
                    fonts.push(std::mem::take(&mut current_font));
                    in_font = false;
                }
                b"cellXfs" => {
                    in_cell_xfs = false;
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
            Ok(Event::Eof) => break,
            Err(source) => {
                return Err(OoxmlError::Xml {
                    part: "xl/styles.xml".to_owned(),
                    source,
                });
            }
            _ => {}
        }
        buf.clear();
    }

    Ok(cell_xfs)
}
