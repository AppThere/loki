// SPDX-License-Identifier: Apache-2.0
// Copyright (C) 2025 AppThere contributors

//! Parsing of `style:page-layout`, `style:header-style`, `style:footer-style`,
//! `style:master-page`, and header/footer paragraph content.

use quick_xml::Reader;
use quick_xml::events::Event;

use crate::error::{OdfError, OdfResult};
use crate::odt::model::document::{OdfHeaderFooterProps, OdfMasterPage, OdfPageLayout};
use crate::odt::model::paragraph::OdfParagraph;
use crate::odt::reader::document::read_paragraph;
use crate::xml_util::local_attr_val;

use super::util::skip_element;

/// Parse a `style:page-layout` element (Start event already consumed).
pub(super) fn parse_page_layout(
    reader: &mut Reader<&[u8]>,
    name: String,
) -> OdfResult<OdfPageLayout> {
    let mut buf = Vec::new();
    let mut layout = OdfPageLayout {
        name,
        page_width: None,
        page_height: None,
        margin_top: None,
        margin_bottom: None,
        margin_left: None,
        margin_right: None,
        print_orientation: None,
        header_props: None,
        footer_props: None,
    };

    loop {
        buf.clear();
        match reader.read_event_into(&mut buf) {
            Ok(Event::Start(ref e)) => {
                let local = e.local_name().into_inner().to_vec();
                match local.as_slice() {
                    b"page-layout-properties" => {
                        layout.page_width = local_attr_val(e, b"page-width");
                        layout.page_height = local_attr_val(e, b"page-height");
                        layout.margin_top = local_attr_val(e, b"margin-top");
                        layout.margin_bottom = local_attr_val(e, b"margin-bottom");
                        layout.margin_left = local_attr_val(e, b"margin-left");
                        layout.margin_right = local_attr_val(e, b"margin-right");
                        layout.print_orientation = local_attr_val(e, b"print-orientation");
                        drop(e);
                        skip_element(reader, b"page-layout-properties")?;
                    }
                    b"header-style" => {
                        drop(e);
                        layout.header_props =
                            Some(parse_header_footer_style(reader, b"header-style")?);
                    }
                    b"footer-style" => {
                        drop(e);
                        layout.footer_props =
                            Some(parse_header_footer_style(reader, b"footer-style")?);
                    }
                    _ => {
                        drop(e);
                        skip_element(reader, &local)?;
                    }
                }
            }
            Ok(Event::Empty(ref e)) => {
                let local = e.local_name().into_inner();
                if local == b"page-layout-properties" {
                    layout.page_width = local_attr_val(e, b"page-width");
                    layout.page_height = local_attr_val(e, b"page-height");
                    layout.margin_top = local_attr_val(e, b"margin-top");
                    layout.margin_bottom = local_attr_val(e, b"margin-bottom");
                    layout.margin_left = local_attr_val(e, b"margin-left");
                    layout.margin_right = local_attr_val(e, b"margin-right");
                    layout.print_orientation = local_attr_val(e, b"print-orientation");
                }
            }
            Ok(Event::End(ref e)) => {
                if e.local_name().into_inner() == b"page-layout" {
                    break;
                }
            }
            Ok(Event::Eof) => break,
            Err(e) => {
                return Err(OdfError::Xml {
                    part: "styles.xml".to_string(),
                    source: e,
                });
            }
            _ => {}
        }
    }

    Ok(layout)
}

/// Parse the `style:header-footer-properties` child of a
/// `style:header-style` / `style:footer-style` element.
pub(super) fn parse_header_footer_style(
    reader: &mut Reader<&[u8]>,
    end_local: &[u8],
) -> OdfResult<OdfHeaderFooterProps> {
    let mut buf = Vec::new();
    let mut props = OdfHeaderFooterProps {
        min_height: None,
        margin: None,
    };

    loop {
        buf.clear();
        match reader.read_event_into(&mut buf) {
            Ok(Event::Start(ref e) | Event::Empty(ref e)) => {
                if e.local_name().into_inner() == b"header-footer-properties" {
                    props.min_height = local_attr_val(e, b"min-height");
                    props.margin = local_attr_val(e, b"margin");
                    if matches!(reader.read_event_into(&mut buf), Ok(Event::End(_))) {}
                }
            }
            Ok(Event::End(ref e)) => {
                if e.local_name().into_inner() == end_local {
                    break;
                }
            }
            Ok(Event::Eof) => break,
            Err(e) => {
                return Err(OdfError::Xml {
                    part: "styles.xml".to_string(),
                    source: e,
                });
            }
            _ => {}
        }
    }

    Ok(props)
}

/// Parse a `style:master-page` element (Start event already consumed).
///
/// Handles all six header/footer variants:
/// - `style:header` / `style:footer` — default (odd/right-page)
/// - `style:header-first` / `style:footer-first` — first page only
/// - `style:header-left` / `style:footer-left` — even/left pages
///
/// ODF 1.3 §16.9.
pub(super) fn parse_master_page(
    reader: &mut Reader<&[u8]>,
    name: String,
    page_layout_name: String,
) -> OdfResult<OdfMasterPage> {
    let mut buf = Vec::new();
    let mut header: Option<Vec<OdfParagraph>> = None;
    let mut footer: Option<Vec<OdfParagraph>> = None;
    let mut header_first: Option<Vec<OdfParagraph>> = None;
    let mut footer_first: Option<Vec<OdfParagraph>> = None;
    let mut header_even: Option<Vec<OdfParagraph>> = None;
    let mut footer_even: Option<Vec<OdfParagraph>> = None;

    loop {
        buf.clear();
        match reader.read_event_into(&mut buf) {
            Ok(Event::Start(ref e)) => {
                let local = e.local_name().into_inner().to_vec();
                match local.as_slice() {
                    b"header" => {
                        drop(e);
                        header = Some(parse_header_footer_paras(reader, b"header")?);
                    }
                    b"footer" => {
                        drop(e);
                        footer = Some(parse_header_footer_paras(reader, b"footer")?);
                    }
                    b"header-first" => {
                        drop(e);
                        header_first = Some(parse_header_footer_paras(reader, b"header-first")?);
                    }
                    b"footer-first" => {
                        drop(e);
                        footer_first = Some(parse_header_footer_paras(reader, b"footer-first")?);
                    }
                    b"header-left" => {
                        drop(e);
                        header_even = Some(parse_header_footer_paras(reader, b"header-left")?);
                    }
                    b"footer-left" => {
                        drop(e);
                        footer_even = Some(parse_header_footer_paras(reader, b"footer-left")?);
                    }
                    _ => {
                        drop(e);
                        skip_element(reader, &local)?;
                    }
                }
            }
            Ok(Event::End(ref e)) => {
                if e.local_name().into_inner() == b"master-page" {
                    break;
                }
            }
            Ok(Event::Eof) => break,
            Err(e) => {
                return Err(OdfError::Xml {
                    part: "styles.xml".to_string(),
                    source: e,
                });
            }
            _ => {}
        }
    }

    Ok(OdfMasterPage {
        name,
        page_layout_name,
        header,
        footer,
        header_first,
        footer_first,
        header_even,
        footer_even,
    })
}

/// Collect paragraphs inside a `style:header`, `style:footer`, or their
/// `-first` / `-left` variants.
///
/// Delegates to [`read_paragraph`] (the same full inline parser used for body
/// content), so spans, fields (`text:page-number`, `text:date`, etc.), links,
/// and notes are all captured correctly.
pub(super) fn parse_header_footer_paras(
    reader: &mut Reader<&[u8]>,
    end_local: &[u8],
) -> OdfResult<Vec<OdfParagraph>> {
    let mut buf = Vec::new();
    let mut paras = Vec::new();

    loop {
        buf.clear();
        match reader.read_event_into(&mut buf) {
            Ok(Event::Start(ref e)) => {
                let local = e.local_name().into_inner().to_vec();
                match local.as_slice() {
                    b"p" | b"h" => {
                        let para = read_paragraph(reader, e)?;
                        paras.push(para);
                    }
                    _ => {
                        drop(e);
                        skip_element(reader, &local)?;
                    }
                }
            }
            Ok(Event::End(ref e)) => {
                if e.local_name().into_inner() == end_local {
                    break;
                }
            }
            Ok(Event::Eof) => break,
            Err(e) => {
                return Err(OdfError::Xml {
                    part: "styles.xml".to_string(),
                    source: e,
                });
            }
            _ => {}
        }
    }

    Ok(paras)
}
