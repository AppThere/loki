// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 AppThere Loki contributors

//! Body-level child dispatcher (`text:section` + the shared body-children
//! loop), split out of `document.rs` for the 300-line ceiling. `read_section`
//! and `read_body_children` are mutually recursive; `read_document` (in the
//! parent) drives the top level through `read_body_children`.
// `drop(ref_binding)` is a deliberate NLL-boundary hint with no runtime effect;
// silence the suggestion to use `let _ = …` instead.
#![allow(dropping_references)]

use quick_xml::Reader;
use quick_xml::events::{BytesStart, Event};

use crate::error::{OdfError, OdfResult};
use crate::odt::model::document::{OdfBodyChild, OdfSection};
use crate::xml_util::local_attr_val;

use super::{read_list, read_paragraph, read_table, read_toc, skip_element};

/// Parse a `text:section` element. ODF 1.3 §5.4.
fn read_section(reader: &mut Reader<&[u8]>, tag: &BytesStart<'_>) -> OdfResult<OdfSection> {
    let name = local_attr_val(tag, b"name");
    let style_name = local_attr_val(tag, b"style-name");
    drop(tag);
    let children = read_body_children(reader, b"section")?;
    Ok(OdfSection {
        name,
        style_name,
        children,
    })
}

// ── Body children shared dispatcher ──────────────────────────────────────────

/// Read body-level children until the `End` event whose local name matches
/// `end_tag`.
///
/// Dispatches `text:p`, `text:h`, `text:list`, `table:table`,
/// `text:table-of-content`, `text:section`, and silently skips everything
/// else. ODF 1.3 §3.1.
pub(super) fn read_body_children(
    reader: &mut Reader<&[u8]>,
    end_tag: &[u8],
) -> OdfResult<Vec<OdfBodyChild>> {
    let mut children: Vec<OdfBodyChild> = Vec::new();
    let mut buf = Vec::new();
    loop {
        buf.clear();
        match reader.read_event_into(&mut buf) {
            Ok(Event::Start(ref e)) => {
                let local = e.local_name().into_inner().to_vec();
                match local.as_slice() {
                    b"p" => {
                        let para = read_paragraph(reader, e)?;
                        children.push(OdfBodyChild::Paragraph(para));
                    }
                    b"h" => {
                        let para = read_paragraph(reader, e)?;
                        children.push(OdfBodyChild::Heading(para));
                    }
                    b"list" => {
                        let list = read_list(reader, e, None, 0)?;
                        children.push(OdfBodyChild::List(list));
                    }
                    b"table" => {
                        let table = read_table(reader, e)?;
                        children.push(OdfBodyChild::Table(table));
                    }
                    b"table-of-content" => {
                        let toc = read_toc(reader, e)?;
                        children.push(OdfBodyChild::TableOfContent(toc));
                    }
                    b"section" => {
                        let section = read_section(reader, e)?;
                        children.push(OdfBodyChild::Section(section));
                    }
                    b"tracked-changes" => {
                        let regions = crate::odt::reader::revisions::read_tracked_changes(reader)?;
                        children.push(OdfBodyChild::TrackedChanges(regions));
                    }
                    b"alphabetical-index"
                    | b"illustration-index"
                    | b"table-index"
                    | b"user-index" => {
                        let element = String::from_utf8_lossy(&local).into_owned();
                        skip_element(reader)?;
                        children.push(OdfBodyChild::Other { element });
                    }
                    _ => {
                        drop(e);
                        skip_element(reader)?;
                    }
                }
            }
            // Empty block-level elements (e.g. text:soft-page-break) are ignored
            // by the catch-all below.
            Ok(Event::End(ref e)) => {
                if e.local_name().into_inner() == end_tag {
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
    Ok(children)
}
