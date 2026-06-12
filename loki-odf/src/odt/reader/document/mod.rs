// SPDX-License-Identifier: Apache-2.0
// Copyright (C) 2025 AppThere contributors

//! Reader for `content.xml` — paragraph-level, inline-level, and body-level
//! parsing.
//!
//! # Caller contract
//!
//! Every `read_X(reader, tag)` function is called **after** its opening
//! `Start` event has been consumed. It reads until — and including — the
//! matching `End` event at the same nesting depth.
// Functions are not yet called from outside this module; suppress lint.
#![allow(dead_code)]
// `drop(ref_binding)` is a deliberate NLL-boundary hint that has no runtime
// effect; silence the suggestion to use `let _ = …` instead.
#![allow(dropping_references)]

mod frame;
mod inline;
mod list;
mod note;
mod table;
pub(crate) mod util;

#[cfg(test)]
mod document_tests;

#[allow(unused_imports)]
pub(crate) use frame::read_frame;
pub(crate) use list::{read_list, read_toc};
pub(crate) use table::read_table;
pub(crate) use util::skip_element;
#[allow(unused_imports)]
pub(crate) use util::read_text_content;

use quick_xml::Reader;
use quick_xml::events::{BytesStart, Event};

use crate::error::{OdfError, OdfResult};
use crate::odt::model::document::{OdfBodyChild, OdfDocument, OdfSection};
use crate::odt::model::paragraph::OdfParagraph;
use crate::version::OdfVersion;
use crate::xml_util::local_attr_val;

use self::inline::read_inline_children;

// ── Paragraph ─────────────────────────────────────────────────────────────────

/// Parse a `text:p` or `text:h` element.
///
/// Called after consuming the `Start` event. `tag` carries the element
/// attributes (`text:style-name`, `text:outline-level`). On return the
/// matching `End` event has been consumed. ODF 1.3 §5.1.
pub(crate) fn read_paragraph(
    reader: &mut Reader<&[u8]>,
    tag: &BytesStart<'_>,
) -> OdfResult<OdfParagraph> {
    let local = tag.local_name().into_inner();
    let is_heading = local == b"h";
    let style_name = local_attr_val(tag, b"style-name");
    let outline_level: Option<u8> =
        local_attr_val(tag, b"outline-level").and_then(|s| s.parse().ok());

    let children = read_inline_children(reader)?;

    Ok(OdfParagraph {
        style_name,
        outline_level,
        is_heading,
        children,
        list_context: None,
    })
}

// ── Section ───────────────────────────────────────────────────────────────────

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
fn read_body_children(reader: &mut Reader<&[u8]>, end_tag: &[u8]) -> OdfResult<Vec<OdfBodyChild>> {
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
            Ok(Event::Empty(ref e)) => {
                // text:soft-page-break may appear between block elements
                let local = e.local_name().into_inner();
                if local != b"soft-page-break" {
                    // ignore other empty block-level elements
                }
            }
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

// ── Document entry point ──────────────────────────────────────────────────────

/// Parse `content.xml` bytes and return the top-level [`OdfDocument`].
///
/// Reads the `office:version` attribute from `office:document-content` (or
/// `office:document`) and the body children from `office:text`. All other
/// top-level sections (`office:automatic-styles`, `office:font-face-decls`,
/// etc.) are skipped here — they are read separately by the importer via
/// [`super::styles::read_stylesheet`] and [`super::styles::read_auto_styles`].
///
/// ODF 1.3 §3.1.
pub(crate) fn read_document(xml: &[u8]) -> OdfResult<OdfDocument> {
    let mut reader = Reader::from_reader(xml);
    reader.config_mut().trim_text(false);
    let mut buf = Vec::new();
    let mut version = OdfVersion::V1_1;
    let mut version_was_absent = true;
    let mut body_children: Vec<OdfBodyChild> = Vec::new();

    loop {
        buf.clear();
        match reader.read_event_into(&mut buf) {
            Ok(Event::Start(ref e)) => {
                let local = e.local_name().into_inner().to_vec();
                match local.as_slice() {
                    b"document-content" | b"document" => {
                        if let Some(v) = local_attr_val(e, b"version") {
                            version_was_absent = false;
                            version = OdfVersion::from_attr(&v).unwrap_or(OdfVersion::V1_3);
                        }
                        // do not skip — descend into children
                    }
                    b"text" => {
                        // office:text — the document body
                        drop(e);
                        body_children = read_body_children(&mut reader, b"text")?;
                    }
                    // office:body is a thin wrapper around office:text;
                    // descend without skipping
                    b"body" => {}
                    // Skip all other top-level sections
                    _ => {
                        drop(e);
                        skip_element(&mut reader)?;
                    }
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

    Ok(OdfDocument {
        version,
        version_was_absent,
        body_children,
    })
}
