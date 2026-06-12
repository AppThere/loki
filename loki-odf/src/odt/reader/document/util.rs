// SPDX-License-Identifier: Apache-2.0
// Copyright (C) 2025 AppThere contributors

//! Utility helpers shared across document-reader submodules.

use quick_xml::Reader;
use quick_xml::events::Event;

use crate::error::{OdfError, OdfResult};

/// Skip all events until the end of the current element.
///
/// Must be called immediately after consuming the `Start` event for the
/// element to skip. Tracks nesting depth so that child elements with the
/// same local name are handled correctly. On return the matching `End`
/// event has been consumed.
pub(crate) fn skip_element(reader: &mut Reader<&[u8]>) -> OdfResult<()> {
    let mut buf = Vec::new();
    let mut depth: u32 = 1;
    loop {
        buf.clear();
        match reader.read_event_into(&mut buf) {
            Ok(Event::Start(_)) => depth += 1,
            Ok(Event::End(_)) => {
                depth -= 1;
                if depth == 0 {
                    return Ok(());
                }
            }
            Ok(Event::Eof) => return Ok(()),
            Err(e) => {
                return Err(OdfError::Xml {
                    part: "content.xml".to_string(),
                    source: e,
                });
            }
            _ => {}
        }
    }
}

/// Collect all text-node content inside the current element.
///
/// Must be called immediately after consuming the `Start` event. Only
/// top-level (depth-1) text nodes are collected; text inside child elements
/// is silently skipped. On return the matching `End` event has been consumed.
pub(crate) fn read_text_content(reader: &mut Reader<&[u8]>) -> OdfResult<String> {
    let mut buf = Vec::new();
    let mut depth: u32 = 1;
    let mut text = String::new();
    loop {
        buf.clear();
        match reader.read_event_into(&mut buf) {
            Ok(Event::Start(_)) => depth += 1,
            Ok(Event::End(_)) => {
                depth -= 1;
                if depth == 0 {
                    return Ok(text);
                }
            }
            Ok(Event::Text(ref t)) if depth == 1 => {
                let s = t.unescape().map_err(|e| OdfError::Xml {
                    part: "content.xml".to_string(),
                    source: e,
                })?;
                text.push_str(&s);
            }
            Ok(Event::Eof) => return Ok(text),
            Err(e) => {
                return Err(OdfError::Xml {
                    part: "content.xml".to_string(),
                    source: e,
                });
            }
            _ => {}
        }
    }
}
