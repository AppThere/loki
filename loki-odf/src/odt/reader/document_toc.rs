// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 AppThere Loki contributors

//! `text:table-of-content` reading for the ODT content reader (split from
//! `document.rs` for the 300-line ceiling): captures the source outline level
//! and the pre-rendered index-body paragraphs. Reaches back via
//! `super::{read_paragraph, skip_element}`; `read_toc` is re-exported from
//! `document.rs`.
#![allow(dropping_references)]

use quick_xml::Reader;
use quick_xml::events::{BytesStart, Event};

use super::{read_paragraph, skip_element};
use crate::error::{OdfError, OdfResult};
use crate::odt::model::document::OdfTableOfContent;
use crate::odt::model::paragraph::OdfParagraph;
use crate::xml_util::local_attr_val;

/// Parse a `text:table-of-content` element. ODF 1.3 §7.5.
pub(crate) fn read_toc(
    reader: &mut Reader<&[u8]>,
    tag: &BytesStart<'_>,
) -> OdfResult<OdfTableOfContent> {
    let name = local_attr_val(tag, b"name");
    let mut source_outline_level: u8 = 3;
    let mut body_paragraphs: Vec<OdfParagraph> = Vec::new();
    let mut buf = Vec::new();
    loop {
        buf.clear();
        match reader.read_event_into(&mut buf) {
            Ok(Event::Start(ref e)) => {
                let local = e.local_name().into_inner().to_vec();
                match local.as_slice() {
                    b"table-of-content-source" => {
                        source_outline_level = local_attr_val(e, b"outline-level")
                            .and_then(|s| s.parse().ok())
                            .unwrap_or(3);
                        drop(e);
                        skip_element(reader)?;
                    }
                    b"index-body" => {
                        drop(e);
                        read_index_body(reader, &mut body_paragraphs)?;
                    }
                    _ => {
                        drop(e);
                        skip_element(reader)?;
                    }
                }
            }
            Ok(Event::Empty(ref e)) => {
                if e.local_name().into_inner() == b"table-of-content-source" {
                    source_outline_level = local_attr_val(e, b"outline-level")
                        .and_then(|s| s.parse().ok())
                        .unwrap_or(3);
                }
            }
            Ok(Event::End(_) | Event::Eof) => break,
            Err(e) => {
                return Err(OdfError::Xml {
                    part: "content.xml".to_string(),
                    source: e,
                });
            }
            _ => {}
        }
    }
    Ok(OdfTableOfContent {
        name,
        source_outline_level,
        body_paragraphs,
    })
}

/// Read `text:p` / `text:h` paragraphs inside `text:index-body`.
fn read_index_body(
    reader: &mut Reader<&[u8]>,
    paragraphs: &mut Vec<OdfParagraph>,
) -> OdfResult<()> {
    let mut buf = Vec::new();
    loop {
        buf.clear();
        match reader.read_event_into(&mut buf) {
            Ok(Event::Start(ref e)) => {
                let local = e.local_name().into_inner();
                if local == b"p" || local == b"h" {
                    paragraphs.push(read_paragraph(reader, e)?);
                } else {
                    drop(e);
                    skip_element(reader)?;
                }
            }
            Ok(Event::End(_) | Event::Eof) => break,
            Err(e) => {
                return Err(OdfError::Xml {
                    part: "content.xml".to_string(),
                    source: e,
                });
            }
            _ => {}
        }
    }
    Ok(())
}
