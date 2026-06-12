// SPDX-License-Identifier: Apache-2.0
// Copyright (C) 2025 AppThere contributors

//! Parsers for `w:sectPr`, `w:pBdr`, `w:tabs`, `w:drawing`, and the
//! `skip_element` utility.

use quick_xml::Reader;
use quick_xml::events::Event;

use crate::docx::model::paragraph::{
    DocxBorderEdge, DocxDrawing, DocxHdrFtrRef, DocxPBdr, DocxPgMar, DocxPgSz, DocxSectPr,
    DocxTab,
};
use crate::docx::reader::util::{attr_val, local_name, parse_emu};
use crate::error::{OoxmlError, OoxmlResult};

/// Parses a `w:sectPr` element. Called after Start("sectPr") is consumed.
pub(crate) fn parse_sect_pr(reader: &mut Reader<&[u8]>) -> OoxmlResult<DocxSectPr> {
    let mut sect = DocxSectPr::default();
    let mut buf = Vec::new();
    loop {
        match reader.read_event_into(&mut buf) {
            Ok(Event::Empty(ref e) | Event::Start(ref e)) => {
                match local_name(e.local_name().as_ref()) {
                    b"pgSz" => {
                        sect.pg_sz = Some(DocxPgSz {
                            w: attr_val(e, b"w")
                                .and_then(|v| v.parse().ok())
                                .unwrap_or(12240),
                            h: attr_val(e, b"h")
                                .and_then(|v| v.parse().ok())
                                .unwrap_or(15840),
                            orient: attr_val(e, b"orient"),
                        });
                    }
                    b"pgMar" => {
                        sect.pg_mar = Some(DocxPgMar {
                            top: attr_val(e, b"top")
                                .and_then(|v| v.parse().ok())
                                .unwrap_or(1440),
                            bottom: attr_val(e, b"bottom")
                                .and_then(|v| v.parse().ok())
                                .unwrap_or(1440),
                            left: attr_val(e, b"left")
                                .and_then(|v| v.parse().ok())
                                .unwrap_or(1440),
                            right: attr_val(e, b"right")
                                .and_then(|v| v.parse().ok())
                                .unwrap_or(1440),
                            header: attr_val(e, b"header")
                                .and_then(|v| v.parse().ok())
                                .unwrap_or(720),
                            footer: attr_val(e, b"footer")
                                .and_then(|v| v.parse().ok())
                                .unwrap_or(720),
                            gutter: attr_val(e, b"gutter")
                                .and_then(|v| v.parse().ok())
                                .unwrap_or(0),
                        });
                    }
                    b"headerReference" => {
                        if let (Some(hf_type), Some(rel_id)) =
                            (attr_val(e, b"type"), attr_val(e, b"id"))
                        {
                            sect.header_refs.push(DocxHdrFtrRef { hf_type, rel_id });
                        }
                    }
                    b"footerReference" => {
                        if let (Some(hf_type), Some(rel_id)) =
                            (attr_val(e, b"type"), attr_val(e, b"id"))
                        {
                            sect.footer_refs.push(DocxHdrFtrRef { hf_type, rel_id });
                        }
                    }
                    b"titlePg" => {
                        // Presence enables first-page variant; w:val="0" disables.
                        sect.title_page = attr_val(e, b"val")
                            .is_none_or(|v| !matches!(v.as_str(), "0" | "false" | "off"));
                    }
                    _ => {}
                }
            }
            Ok(Event::End(ref e)) if local_name(e.local_name().as_ref()) == b"sectPr" => {
                break;
            }
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
    Ok(sect)
}

/// Parses a `w:pBdr` element. Called after Start("pBdr") is consumed.
pub(super) fn parse_pbdr(reader: &mut Reader<&[u8]>) -> OoxmlResult<DocxPBdr> {
    let mut pbdr = DocxPBdr::default();
    let mut buf = Vec::new();
    loop {
        match reader.read_event_into(&mut buf) {
            Ok(Event::Empty(ref e) | Event::Start(ref e)) => {
                let edge = DocxBorderEdge {
                    val: attr_val(e, b"val").unwrap_or_default(),
                    sz: attr_val(e, b"sz").and_then(|v| v.parse().ok()),
                    color: attr_val(e, b"color"),
                    space: attr_val(e, b"space").and_then(|v| v.parse().ok()),
                };
                match local_name(e.local_name().as_ref()) {
                    b"top" => pbdr.top = Some(edge),
                    b"bottom" => pbdr.bottom = Some(edge),
                    b"left" => pbdr.left = Some(edge),
                    b"right" => pbdr.right = Some(edge),
                    b"between" => pbdr.between = Some(edge),
                    _ => {}
                }
            }
            Ok(Event::End(ref e)) if local_name(e.local_name().as_ref()) == b"pBdr" => {
                break;
            }
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
    Ok(pbdr)
}

/// Parses a `w:tabs` element and appends each `w:tab` to `out`.
/// Called after Start("tabs") is consumed.
pub(super) fn parse_tabs(reader: &mut Reader<&[u8]>, out: &mut Vec<DocxTab>) -> OoxmlResult<()> {
    let mut buf = Vec::new();
    loop {
        match reader.read_event_into(&mut buf) {
            Ok(Event::Empty(ref e) | Event::Start(ref e))
                if local_name(e.local_name().as_ref()) == b"tab" =>
            {
                if let Some(val) = attr_val(e, b"val") {
                    out.push(DocxTab {
                        val,
                        pos: attr_val(e, b"pos")
                            .and_then(|v| v.parse().ok())
                            .unwrap_or(0),
                        leader: attr_val(e, b"leader"),
                    });
                }
            }
            Ok(Event::End(ref e)) if local_name(e.local_name().as_ref()) == b"tabs" => {
                break;
            }
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

/// Parses a `w:drawing` element. Called after Start("drawing") is consumed.
pub(super) fn parse_drawing(reader: &mut Reader<&[u8]>) -> OoxmlResult<DocxDrawing> {
    let mut drawing = DocxDrawing {
        rel_id: None,
        cx: None,
        cy: None,
        descr: None,
        name: None,
        is_anchor: false,
    };
    let mut buf = Vec::new();
    loop {
        match reader.read_event_into(&mut buf) {
            Ok(Event::Start(ref e) | Event::Empty(ref e)) => {
                match local_name(e.local_name().as_ref()) {
                    b"anchor" => drawing.is_anchor = true,
                    b"extent" => {
                        drawing.cx = attr_val(e, b"cx").as_deref().and_then(parse_emu);
                        drawing.cy = attr_val(e, b"cy").as_deref().and_then(parse_emu);
                    }
                    b"docPr" => {
                        drawing.descr = attr_val(e, b"descr");
                        drawing.name = attr_val(e, b"name");
                    }
                    b"blip" => {
                        drawing.rel_id = attr_val(e, b"embed");
                    }
                    _ => {}
                }
            }
            Ok(Event::End(ref e)) if local_name(e.local_name().as_ref()) == b"drawing" => {
                break;
            }
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
    Ok(drawing)
}

/// Skips all content inside an element until its matching end tag.
pub(crate) fn skip_element(reader: &mut Reader<&[u8]>, end_tag: &[u8]) -> OoxmlResult<()> {
    let mut depth = 1i32;
    let mut buf = Vec::new();
    loop {
        match reader.read_event_into(&mut buf) {
            Ok(Event::Start(_)) => depth += 1,
            Ok(Event::End(ref e)) => {
                depth -= 1;
                if depth == 0 && local_name(e.local_name().as_ref()) == end_tag {
                    break;
                }
            }
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
