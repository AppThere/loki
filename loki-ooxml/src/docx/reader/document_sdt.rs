// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 AppThere Loki contributors

//! Reader for block-level `w:sdt` (structured document tag / content control).
//!
//! Loki does not model content controls, but a block `w:sdt`'s `w:sdtContent`
//! wraps real body content (paragraphs, tables, nested content controls).
//! Skipping the `w:sdt` outright — as the reader used to — dropped that content
//! (data loss for the many Word documents whose cover pages, forms, and headings
//! sit inside content controls). This unwraps the content into the surrounding
//! body: the control's chrome (binding, placeholder, tag in `w:sdtPr`) is
//! discarded, the content preserved.

use quick_xml::{Reader, events::Event};

use crate::docx::model::document::DocxBodyChild;
use crate::docx::reader::util::local_name;
use crate::error::{OoxmlError, OoxmlResult};

use super::table;
use super::{parse_paragraph, skip_element};

/// Parses a block-level `w:sdt`, appending its `w:sdtContent` children
/// (paragraphs, tables, nested `w:sdt`) to `children`. Called after the
/// `Start(w:sdt)` event is consumed. Non-content children (`w:sdtPr`,
/// `w:sdtEndPr`) are ignored — `in_content` gates on `w:sdtContent` so their
/// inner elements never reach the dispatch.
pub(super) fn parse_sdt(
    reader: &mut Reader<&[u8]>,
    children: &mut Vec<DocxBodyChild>,
) -> OoxmlResult<()> {
    let mut buf = Vec::new();
    let mut in_content = false;
    loop {
        match reader.read_event_into(&mut buf) {
            Ok(Event::Start(ref e)) => match local_name(e.local_name().as_ref()) {
                b"sdtContent" => in_content = true,
                b"p" if in_content => {
                    children.push(DocxBodyChild::Paragraph(parse_paragraph(reader)?));
                }
                b"tbl" if in_content => {
                    children.push(DocxBodyChild::Table(table::parse_table(reader)?));
                }
                b"sdt" if in_content => parse_sdt(reader, children)?,
                b"sdtPr" | b"sdtEndPr" => {
                    // Skip the control's properties wholesale so their inner
                    // `w:tag`/`w:alias`/binding elements never reach the dispatch.
                    let name = local_name(e.local_name().as_ref()).to_vec();
                    skip_element(reader, &name)?;
                }
                _ => {}
            },
            Ok(Event::End(ref e)) if local_name(e.local_name().as_ref()) == b"sdtContent" => {
                in_content = false;
            }
            Ok(Event::End(ref e)) if local_name(e.local_name().as_ref()) == b"sdt" => break,
            Ok(Event::Eof) => break,
            Err(e) => {
                return Err(OoxmlError::Xml {
                    part: "word/document.xml".into(),
                    source: e,
                });
            }
            _ => {}
        }
        buf.clear();
    }
    Ok(())
}
