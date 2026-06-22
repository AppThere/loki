// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 AppThere Loki contributors

//! Reader for `office:annotation` (comment) bodies. ODF 1.3 §14.1.

use quick_xml::Reader;
use quick_xml::events::{BytesStart, Event};

use crate::error::{OdfError, OdfResult};
use crate::odt::model::paragraph::OdfParagraphChild;
use crate::xml_util::local_attr_val;

/// Parses an `office:annotation` element (the `Start` event already consumed)
/// into an [`OdfParagraphChild::Annotation`]. Collects `dc:creator`, `dc:date`,
/// and the plain text of the body paragraphs (joined by `\n`).
pub(crate) fn read_annotation(
    reader: &mut Reader<&[u8]>,
    e: &BytesStart<'_>,
) -> OdfResult<OdfParagraphChild> {
    let name = local_attr_val(e, b"name");
    let mut creator = None;
    let mut date = None;
    let mut body = String::new();

    let mut buf = Vec::new();
    // Which metadata element's text we are collecting (`creator` / `date`), or
    // `None` while inside a body paragraph.
    let mut collecting: Option<&'static str> = None;
    let mut meta_text = String::new();
    let mut in_paragraph = false;
    let mut first_paragraph = true;

    loop {
        buf.clear();
        match reader.read_event_into(&mut buf) {
            Ok(Event::Start(ref c)) => match c.local_name().into_inner() {
                b"creator" => {
                    collecting = Some("creator");
                    meta_text.clear();
                }
                b"date" => {
                    collecting = Some("date");
                    meta_text.clear();
                }
                b"p" => {
                    if !first_paragraph {
                        body.push('\n');
                    }
                    first_paragraph = false;
                    in_paragraph = true;
                }
                _ => {}
            },
            Ok(Event::Text(ref t)) => {
                let s = t.unescape().map_err(xml_err)?;
                if collecting.is_some() {
                    meta_text.push_str(&s);
                } else if in_paragraph {
                    body.push_str(&s);
                }
            }
            Ok(Event::End(ref c)) => match c.local_name().into_inner() {
                b"creator" => {
                    creator = Some(std::mem::take(&mut meta_text));
                    collecting = None;
                }
                b"date" => {
                    date = Some(std::mem::take(&mut meta_text));
                    collecting = None;
                }
                b"p" => in_paragraph = false,
                b"annotation" => break,
                _ => {}
            },
            Ok(Event::Eof) => break,
            Err(e) => return Err(xml_err(e)),
            _ => {}
        }
    }

    Ok(OdfParagraphChild::Annotation {
        name,
        creator,
        date,
        body,
    })
}

fn xml_err(source: quick_xml::Error) -> OdfError {
    OdfError::Xml {
        part: "content.xml".to_string(),
        source,
    }
}
