// SPDX-License-Identifier: Apache-2.0
// Copyright (C) 2025 AppThere contributors

//! Parser for `text:note` (footnote / endnote) elements.

use quick_xml::Reader;
use quick_xml::events::Event;

use crate::error::{OdfError, OdfResult};
use crate::odt::model::notes::{OdfNote, OdfNoteClass};
use crate::odt::model::paragraph::OdfParagraph;

use super::read_paragraph;
use super::util::{read_text_content, skip_element};

/// Parse the body of a `text:note` element.
///
/// Called after the `Start` event for `text:note` has been consumed and
/// `id` / `note_class` have been extracted from its attributes. Reads until
/// the matching `</text:note>` end tag. ODF 1.3 §6.3.
pub(super) fn read_note_body(
    reader: &mut Reader<&[u8]>,
    id: Option<String>,
    note_class: OdfNoteClass,
) -> OdfResult<OdfNote> {
    let mut citation: Option<String> = None;
    let mut body: Vec<OdfParagraph> = Vec::new();
    let mut buf = Vec::new();

    loop {
        buf.clear();
        match reader.read_event_into(&mut buf) {
            Ok(Event::Start(ref e)) => {
                let local = e.local_name().into_inner().to_vec();
                match local.as_slice() {
                    b"note-citation" => {
                        drop(e);
                        citation = Some(read_text_content(reader)?);
                    }
                    b"note-body" => {
                        drop(e); // children parsed by subsequent iterations
                    }
                    b"p" | b"h" => {
                        let para = read_paragraph(reader, e)?;
                        body.push(para);
                    }
                    _ => {
                        drop(e);
                        skip_element(reader)?;
                    }
                }
            }
            Ok(Event::End(ref e)) => {
                if e.local_name().into_inner() == b"note" {
                    break;
                }
            }
            Ok(Event::Eof) => break,
            Err(e) => {
                return Err(OdfError::Xml {
                    part: "content.xml".to_string(),
                    source: e,
                });
            }
            _ => {}
        }
    }

    Ok(OdfNote {
        id,
        note_class,
        citation,
        body,
    })
}
