// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 AppThere Loki contributors

//! Shared strings XML parsing.

use crate::error::OoxmlError;
use crate::xml_util::local_name;
use quick_xml::Reader;
use quick_xml::events::Event;

pub(super) fn parse_shared_strings(data: &[u8]) -> Result<Vec<String>, OoxmlError> {
    let mut reader = Reader::from_reader(data);
    reader.config_mut().trim_text(false);
    let mut buf = Vec::new();
    let mut strings = Vec::new();
    let mut current_string = String::new();
    let mut in_t = false;

    loop {
        match reader.read_event_into(&mut buf) {
            Ok(Event::Start(ref e)) => {
                if local_name(e) == b"t" {
                    in_t = true;
                }
            }
            Ok(Event::End(ref e)) => {
                let name_bytes = e.local_name().into_inner();
                let name = if let Some(pos) = name_bytes.iter().position(|&b| b == b':') {
                    &name_bytes[pos + 1..]
                } else {
                    name_bytes
                };
                if name == b"t" {
                    in_t = false;
                } else if name == b"si" {
                    strings.push(std::mem::take(&mut current_string));
                }
            }
            Ok(Event::Text(ref e)) => {
                if in_t {
                    current_string.push_str(&e.unescape().unwrap_or_default());
                }
            }
            Ok(Event::Eof) => break,
            Err(source) => {
                return Err(OoxmlError::Xml {
                    part: "xl/sharedStrings.xml".to_owned(),
                    source,
                });
            }
            _ => {}
        }
        buf.clear();
    }
    Ok(strings)
}
