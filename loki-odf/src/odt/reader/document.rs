// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 AppThere Loki contributors

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

use quick_xml::Reader;
use quick_xml::events::{BytesStart, Event};

use crate::error::{OdfError, OdfResult};
use crate::odt::model::document::{OdfBodyChild, OdfDocument};
use crate::odt::model::notes::{OdfNote, OdfNoteClass};
use crate::odt::model::paragraph::OdfParagraph;
use crate::version::OdfVersion;
use crate::xml_util::{event_text, local_attr_val};

use super::inlines::read_inline_children;

#[path = "document_frame.rs"]
mod frame;
pub(super) use frame::read_frame_kind;
#[path = "document_list.rs"]
mod list;
pub(crate) use list::read_list;
#[path = "document_table.rs"]
mod table;
pub(crate) use table::read_table;
#[path = "document_toc.rs"]
mod toc;
pub(crate) use toc::read_toc;
#[path = "document_body.rs"]
mod body;
use body::read_body_children;

// ── Utilities ─────────────────────────────────────────────────────────────────

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
            Ok(ref ev @ (Event::Text(_) | Event::GeneralRef(_))) if depth == 1 => {
                text.push_str(&event_text(ev).map_err(|e| OdfError::Xml {
                    part: "content.xml".to_string(),
                    source: e,
                })?);
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

    let children = read_inline_children(reader, 0)?;

    Ok(OdfParagraph {
        style_name,
        outline_level,
        is_heading,
        children,
        list_context: None,
    })
}

// ── Note ──────────────────────────────────────────────────────────────────────

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

// ── Tests ──────────────────────────────────────────────────────────────────────

#[cfg(test)]
#[path = "document_tests.rs"]
mod tests;
