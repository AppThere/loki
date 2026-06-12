// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 AppThere Loki contributors

//! Workbook XML parsing — extracts the sheet list.

use crate::error::OoxmlError;
use crate::xml_util::{local_attr_val, local_name};
use quick_xml::Reader;
use quick_xml::events::Event;

pub(super) struct RawSheet {
    pub(super) name: String,
    pub(super) rel_id: String,
}

pub(super) fn parse_workbook_sheets(data: &[u8]) -> Result<Vec<RawSheet>, OoxmlError> {
    let mut reader = Reader::from_reader(data);
    reader.config_mut().trim_text(false);
    let mut buf = Vec::new();
    let mut sheets = Vec::new();

    loop {
        match reader.read_event_into(&mut buf) {
            Ok(Event::Start(ref e)) | Ok(Event::Empty(ref e)) => {
                if local_name(e) == b"sheet" {
                    if let (Some(name), Some(rel_id)) =
                        (local_attr_val(e, b"name"), local_attr_val(e, b"id"))
                    {
                        sheets.push(RawSheet { name, rel_id });
                    }
                }
            }
            Ok(Event::Eof) => break,
            Err(source) => {
                return Err(OoxmlError::Xml {
                    part: "xl/workbook.xml".to_owned(),
                    source,
                });
            }
            _ => {}
        }
        buf.clear();
    }
    Ok(sheets)
}
