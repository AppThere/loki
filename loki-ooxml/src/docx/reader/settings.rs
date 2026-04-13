// Copyright 2026 AppThere Loki contributors
// SPDX-License-Identifier: Apache-2.0

//! Reader for `word/settings.xml` → [`DocxSettings`].
//!
//! ECMA-376 §17.15 (document settings).

use quick_xml::events::Event;
use quick_xml::Reader;

use crate::docx::model::settings::DocxSettings;
use crate::docx::reader::util::{attr_val, local_name};
use crate::error::{OoxmlError, OoxmlResult};

/// Parses `word/settings.xml` into a [`DocxSettings`] model.
///
/// ECMA-376 §17.15.1.78.
pub fn parse_settings(xml: &[u8]) -> OoxmlResult<DocxSettings> {
    let mut reader = Reader::from_reader(xml);
    reader.config_mut().trim_text(false);

    let mut result = DocxSettings::default();
    let mut buf = Vec::new();

    loop {
        match reader.read_event_into(&mut buf) {
            Ok(Event::Empty(ref e)) | Ok(Event::Start(ref e)) => {
                match local_name(e.local_name().as_ref()) {
                    b"defaultTabStop" => {
                        result.default_tab_stop = attr_val(e, b"val")
                            .and_then(|v| v.parse::<i32>().ok());
                    }
                    b"evenAndOddHeaders" => {
                        result.even_and_odd_headers = attr_val(e, b"val")
                            .map_or(true, |v| !matches!(v.as_str(), "0" | "false" | "off"));
                    }
                    b"titlePg" => {
                        result.title_pg = attr_val(e, b"val")
                            .map_or(true, |v| !matches!(v.as_str(), "0" | "false" | "off"));
                    }
                    _ => {}
                }
            }
            Ok(Event::Eof) => break,
            Err(e) => {
                return Err(OoxmlError::Xml {
                    part: "word/settings.xml".into(),
                    source: e,
                });
            }
            _ => {}
        }
        buf.clear();
    }
    Ok(result)
}

#[cfg(test)]
mod tests {
    use super::*;

    const MINIMAL_SETTINGS: &[u8] = br#"<?xml version="1.0" encoding="UTF-8"?>
<w:settings xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main">
  <w:defaultTabStop w:val="720"/>
  <w:evenAndOddHeaders/>
</w:settings>"#;

    #[test]
    fn parses_default_tab_stop() {
        let settings = parse_settings(MINIMAL_SETTINGS).unwrap();
        assert_eq!(settings.default_tab_stop, Some(720));
    }

    #[test]
    fn parses_even_and_odd_headers() {
        let settings = parse_settings(MINIMAL_SETTINGS).unwrap();
        assert!(settings.even_and_odd_headers);
    }

    #[test]
    fn title_pg_defaults_false() {
        let settings = parse_settings(MINIMAL_SETTINGS).unwrap();
        assert!(!settings.title_pg);
    }
}
