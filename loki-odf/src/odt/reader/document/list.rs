// SPDX-License-Identifier: Apache-2.0
// Copyright (C) 2025 AppThere contributors

//! Parsers for `text:list`, `text:list-item`, and `text:table-of-content`.

use quick_xml::Reader;
use quick_xml::events::{BytesStart, Event};

use crate::error::{OdfError, OdfResult};
use crate::odt::model::document::{OdfList, OdfListItem, OdfListItemChild, OdfTableOfContent};
use crate::odt::model::paragraph::{OdfListContext, OdfParagraph};
use crate::xml_util::local_attr_val;

use super::read_paragraph;
use super::util::skip_element;

/// Parse a `text:list` element. ODF 1.3 §5.3.
///
/// Called after consuming the `Start` event. `parent_style` is the inherited
/// style from an enclosing list (used when this list has no explicit
/// `text:style-name`). `depth` is the 0-indexed nesting depth.
pub(crate) fn read_list(
    reader: &mut Reader<&[u8]>,
    tag: &BytesStart<'_>,
    parent_style: Option<&str>,
    depth: u8,
) -> OdfResult<OdfList> {
    let style_name = local_attr_val(tag, b"style-name");
    let xml_id = local_attr_val(tag, b"id");
    let continue_list = local_attr_val(tag, b"continue-list");
    let continue_numbering =
        local_attr_val(tag, b"continue-numbering").is_some_and(|s| s == "true");

    let effective: Option<String> = style_name
        .clone()
        .or_else(|| parent_style.map(String::from));

    let mut items: Vec<OdfListItem> = Vec::new();
    let mut buf = Vec::new();
    loop {
        buf.clear();
        match reader.read_event_into(&mut buf) {
            Ok(Event::Start(ref e)) => {
                let local = e.local_name().into_inner().to_vec();
                match local.as_slice() {
                    b"list-item" | b"list-header" => {
                        let item = read_list_item(reader, e, effective.as_deref(), depth)?;
                        items.push(item);
                    }
                    _ => {
                        drop(e);
                        skip_element(reader)?;
                    }
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
    Ok(OdfList {
        xml_id,
        style_name,
        continue_list,
        continue_numbering,
        items,
    })
}

/// Parse a `text:list-item` or `text:list-header` element. ODF 1.3 §5.3.
fn read_list_item(
    reader: &mut Reader<&[u8]>,
    tag: &BytesStart<'_>,
    list_style: Option<&str>,
    depth: u8,
) -> OdfResult<OdfListItem> {
    let start_value: Option<u32> = local_attr_val(tag, b"start-value").and_then(|s| s.parse().ok());
    let mut children: Vec<OdfListItemChild> = Vec::new();
    let mut buf = Vec::new();
    loop {
        buf.clear();
        match reader.read_event_into(&mut buf) {
            Ok(Event::Start(ref e)) => {
                let local = e.local_name().into_inner().to_vec();
                match local.as_slice() {
                    b"p" => {
                        let mut para = read_paragraph(reader, e)?;
                        para.list_context = Some(OdfListContext {
                            style_name: list_style.map(String::from),
                            level: depth,
                            item_id: None,
                        });
                        children.push(OdfListItemChild::Paragraph(para));
                    }
                    b"h" => {
                        let mut para = read_paragraph(reader, e)?;
                        para.list_context = Some(OdfListContext {
                            style_name: list_style.map(String::from),
                            level: depth,
                            item_id: None,
                        });
                        children.push(OdfListItemChild::Heading(para));
                    }
                    b"list" => {
                        let nested = read_list(reader, e, list_style, depth.saturating_add(1))?;
                        children.push(OdfListItemChild::List(nested));
                    }
                    _ => {
                        drop(e);
                        skip_element(reader)?;
                    }
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
    Ok(OdfListItem {
        start_value,
        children,
    })
}

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
