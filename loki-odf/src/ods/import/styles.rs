// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 AppThere Loki contributors

//! ODS style parsing — collects data styles and cell styles from XML.

use std::collections::HashMap;

use loki_sheet_model::{CellAlign, CellStyle, NumberFormat};
use quick_xml::Reader;
use quick_xml::events::Event;

use crate::xml_util::local_attr_val;

use super::xml_helpers::{local_name, local_name_end};

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
                                if let Some(underline) =
                                    local_attr_val(e, b"text-underline-style")
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
