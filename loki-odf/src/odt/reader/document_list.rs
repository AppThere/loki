// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 AppThere Loki contributors

//! `text:list` / `text:list-item` reading for the ODT content reader (split
//! from `document.rs` for the 300-line ceiling): builds nested `OdfList`s with
//! a stack-depth guard. Paragraph/heading children and the nesting recursion
//! reach back via `super::{read_paragraph, skip_element}`; `read_list` is
//! re-exported from `document.rs` (also used by the table reader).
#![allow(dropping_references)]

use quick_xml::Reader;
use quick_xml::events::{BytesStart, Event};

use super::{read_paragraph, skip_element};
use crate::error::{OdfError, OdfResult};
use crate::limits::MAX_NESTING_DEPTH;
use crate::odt::model::document::{OdfList, OdfListItem, OdfListItemChild};
use crate::odt::model::paragraph::OdfListContext;
use crate::xml_util::local_attr_val;

/// Parse a `text:list` element. ODF 1.3 §5.3.
///
/// Called after consuming the `Start` event. `parent_style` is the inherited
/// style from an enclosing list (used when this list has no explicit
/// `text:style-name`). `depth` is the 0-indexed nesting depth.
///
/// # Errors
///
/// Returns [`OdfError::NestingTooDeep`] when lists are nested beyond
/// [`MAX_NESTING_DEPTH`] (stack-exhaustion guard).
pub(crate) fn read_list(
    reader: &mut Reader<&[u8]>,
    tag: &BytesStart<'_>,
    parent_style: Option<&str>,
    depth: u8,
) -> OdfResult<OdfList> {
    if usize::from(depth) > MAX_NESTING_DEPTH {
        return Err(OdfError::NestingTooDeep {
            limit: MAX_NESTING_DEPTH,
        });
    }
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
