// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 AppThere Loki contributors

//! DOCX-analogue ODT frame parsing: `draw:frame` and its kind (image /
//! text-box / object), image children, and text-box paragraphs (which recurse
//! through `super::read_paragraph`). Split out of `document.rs` (Phase 7.1).
//! ODF 1.3 §10.4.

// `drop(ref_binding)` is a deliberate NLL-boundary hint (see `document.rs`).
#![allow(dropping_references)]
// read_frame is kept for the not-yet-wired inline-frame path (see document.rs).
#![allow(dead_code)]

use quick_xml::Reader;
use quick_xml::events::{BytesStart, Event};

use crate::error::{OdfError, OdfResult};
use crate::odt::model::frames::{OdfFrame, OdfFrameKind};
use crate::odt::model::paragraph::OdfParagraph;
use crate::xml_util::local_attr_val;

use super::{read_paragraph, read_text_content, skip_element};

/// Parse a `draw:frame` element.
///
/// Called after consuming the `Start` event. `tag` carries the frame
/// geometry and style attributes. On return the matching `End` event
/// has been consumed. ODF 1.3 §10.4.
pub(crate) fn read_frame(reader: &mut Reader<&[u8]>, tag: &BytesStart<'_>) -> OdfResult<OdfFrame> {
    let name = local_attr_val(tag, b"name");
    let style_name = local_attr_val(tag, b"style-name");
    let anchor_type = local_attr_val(tag, b"anchor-type");
    let width = local_attr_val(tag, b"width");
    let height = local_attr_val(tag, b"height");
    let x = local_attr_val(tag, b"x");
    let y = local_attr_val(tag, b"y");
    let kind = read_frame_kind(reader)?;
    Ok(OdfFrame {
        name,
        style_name,
        anchor_type,
        width,
        height,
        x,
        y,
        kind,
    })
}

/// Determine the [`OdfFrameKind`] by reading the first recognised child of
/// a `draw:frame` element.
///
/// Called after frame attributes have been extracted. Reads until
/// `</draw:frame>`. ODF 1.3 §10.4–§10.7.
pub(crate) fn read_frame_kind(reader: &mut Reader<&[u8]>) -> OdfResult<OdfFrameKind> {
    let mut kind = OdfFrameKind::Other;
    let mut buf = Vec::new();
    loop {
        buf.clear();
        match reader.read_event_into(&mut buf) {
            Ok(Event::Start(ref e)) => {
                let local = e.local_name().into_inner().to_vec();
                match local.as_slice() {
                    b"image" => {
                        let href = local_attr_val(e, b"href").unwrap_or_default();
                        let media_type = local_attr_val(e, b"type");
                        drop(e);
                        let (title, desc) = read_image_children(reader)?;
                        kind = OdfFrameKind::Image {
                            href,
                            media_type,
                            title,
                            desc,
                        };
                    }
                    b"text-box" => {
                        drop(e);
                        let paragraphs = read_text_box_paragraphs(reader)?;
                        kind = OdfFrameKind::TextBox { paragraphs };
                    }
                    b"object" => {
                        let href = local_attr_val(e, b"href").unwrap_or_default();
                        drop(e);
                        skip_element(reader)?;
                        kind = OdfFrameKind::Object { href };
                    }
                    _ => {
                        drop(e);
                        skip_element(reader)?;
                    }
                }
            }
            Ok(Event::Empty(ref e)) => match e.local_name().into_inner() {
                b"image" => {
                    let href = local_attr_val(e, b"href").unwrap_or_default();
                    let media_type = local_attr_val(e, b"type");
                    kind = OdfFrameKind::Image {
                        href,
                        media_type,
                        title: None,
                        desc: None,
                    };
                }
                b"object" => {
                    let href = local_attr_val(e, b"href").unwrap_or_default();
                    kind = OdfFrameKind::Object { href };
                }
                _ => {}
            },
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
    Ok(kind)
}

/// Read `svg:title` and `svg:desc` children of a `draw:image` element.
///
/// Called after consuming `Start(image)`. Returns `(title, desc)` and
/// positions the reader after `</draw:image>`. ODF 1.3 §10.5.
fn read_image_children(reader: &mut Reader<&[u8]>) -> OdfResult<(Option<String>, Option<String>)> {
    let mut title: Option<String> = None;
    let mut desc: Option<String> = None;
    let mut buf = Vec::new();
    loop {
        buf.clear();
        match reader.read_event_into(&mut buf) {
            Ok(Event::Start(ref e)) => {
                let local = e.local_name().into_inner().to_vec();
                drop(e);
                match local.as_slice() {
                    b"title" => title = Some(read_text_content(reader)?),
                    b"desc" => desc = Some(read_text_content(reader)?),
                    _ => skip_element(reader)?,
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
    Ok((title, desc))
}

/// Read `text:p` / `text:h` children of a `draw:text-box` element.
///
/// Called after consuming `Start(text-box)`. Returns the paragraphs and
/// positions the reader after `</draw:text-box>`. ODF 1.3 §10.7.
fn read_text_box_paragraphs(reader: &mut Reader<&[u8]>) -> OdfResult<Vec<OdfParagraph>> {
    let mut paragraphs = Vec::new();
    let mut buf = Vec::new();
    loop {
        buf.clear();
        match reader.read_event_into(&mut buf) {
            Ok(Event::Start(ref e)) => {
                let local = e.local_name().into_inner();
                if local == b"p" || local == b"h" {
                    let para = read_paragraph(reader, e)?;
                    paragraphs.push(para);
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
    Ok(paragraphs)
}
