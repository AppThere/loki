// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 AppThere Loki contributors

//! `w:pPr` (paragraph-properties) parsing, split out of `document.rs` for the
//! 300-line ceiling: `parse_ppr_element` and its attribute/border/tab helpers.
//! `parse_ppr_element` is re-exported `pub(crate)` from the parent (external
//! callers in `styles.rs` / `numbering.rs`).

use quick_xml::{Reader, events::Event};

use crate::docx::model::paragraph::{
    DocxBorderEdge, DocxFramePr, DocxInd, DocxNumPr, DocxPBdr, DocxPPr, DocxSpacing, DocxTab,
};
use crate::docx::reader::sectpr::parse_sect_pr;
use crate::docx::reader::util::{attr_val, local_name, toggle_prop};
use crate::error::{OoxmlError, OoxmlResult};

use super::parse_rpr_element;

/// Parses a `w:pPr` element. Called after the Start("pPr") event is consumed.
pub(crate) fn parse_ppr_element(reader: &mut Reader<&[u8]>) -> OoxmlResult<DocxPPr> {
    let mut ppr = DocxPPr::default();
    let mut buf = Vec::new();

    loop {
        match reader.read_event_into(&mut buf) {
            Ok(Event::Start(ref e)) => match local_name(e.local_name().as_ref()) {
                b"numPr" => {
                    let mut ilvl = 0u8;
                    let mut num_id = 0u32;
                    let mut nbuf = Vec::new();
                    loop {
                        match reader.read_event_into(&mut nbuf) {
                            Ok(Event::Empty(ref ne) | Event::Start(ref ne)) => {
                                match local_name(ne.local_name().as_ref()) {
                                    b"ilvl" => {
                                        ilvl = attr_val(ne, b"val")
                                            .as_deref()
                                            .and_then(|v| v.parse().ok())
                                            .unwrap_or(0);
                                    }
                                    b"numId" => {
                                        num_id = attr_val(ne, b"val")
                                            .as_deref()
                                            .and_then(|v| v.parse().ok())
                                            .unwrap_or(0);
                                    }
                                    _ => {}
                                }
                            }
                            Ok(Event::End(ref ne))
                                if local_name(ne.local_name().as_ref()) == b"numPr" =>
                            {
                                break;
                            }
                            Ok(Event::Eof) | Err(_) => break,
                            _ => {}
                        }
                        nbuf.clear();
                    }
                    ppr.num_pr = Some(DocxNumPr { ilvl, num_id });
                }
                b"pBdr" => {
                    ppr.p_bdr = Some(parse_pbdr(reader)?);
                }
                b"tabs" => {
                    parse_tabs(reader, &mut ppr.tabs)?;
                }
                b"sectPr" => {
                    ppr.sect_pr = Some(parse_sect_pr(reader)?);
                }
                b"rPr" => {
                    ppr.ppr_rpr = Some(parse_rpr_element(reader)?);
                }
                name => apply_ppr_attr(name, e, &mut ppr),
            },
            Ok(Event::Empty(ref e)) => {
                apply_ppr_attr(local_name(e.local_name().as_ref()), e, &mut ppr);
            }
            Ok(Event::End(ref e)) if local_name(e.local_name().as_ref()) == b"pPr" => {
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
    Ok(ppr)
}

/// Applies a single `w:pPr` attribute-only child element to `ppr`.
fn apply_ppr_attr(name: &[u8], e: &quick_xml::events::BytesStart<'_>, ppr: &mut DocxPPr) {
    match name {
        b"pStyle" => ppr.style_id = attr_val(e, b"val"),
        b"jc" => ppr.jc = attr_val(e, b"val"),
        b"ind" => {
            ppr.ind = Some(DocxInd {
                left: attr_val(e, b"left").as_deref().and_then(|v| v.parse().ok()),
                right: attr_val(e, b"right")
                    .as_deref()
                    .and_then(|v| v.parse().ok()),
                first_line: attr_val(e, b"firstLine")
                    .as_deref()
                    .and_then(|v| v.parse().ok()),
                hanging: attr_val(e, b"hanging")
                    .as_deref()
                    .and_then(|v| v.parse().ok()),
            });
        }
        b"spacing" => {
            ppr.spacing = Some(DocxSpacing {
                before: attr_val(e, b"before")
                    .as_deref()
                    .and_then(|v| v.parse().ok()),
                after: attr_val(e, b"after")
                    .as_deref()
                    .and_then(|v| v.parse().ok()),
                line: attr_val(e, b"line").as_deref().and_then(|v| v.parse().ok()),
                line_rule: attr_val(e, b"lineRule"),
            });
        }
        b"keepLines" => ppr.keep_lines = Some(toggle_prop(attr_val(e, b"val").as_deref())),
        b"keepNext" => ppr.keep_next = Some(toggle_prop(attr_val(e, b"val").as_deref())),
        b"pageBreakBefore" => {
            ppr.page_break_before = Some(toggle_prop(attr_val(e, b"val").as_deref()));
        }
        b"outlineLvl" => {
            ppr.outline_lvl = attr_val(e, b"val")
                .as_deref()
                .and_then(|v| v.parse::<u8>().ok());
        }
        b"bidi" => ppr.bidi = Some(toggle_prop(attr_val(e, b"val").as_deref())),
        b"widowControl" => ppr.widow_control = Some(toggle_prop(attr_val(e, b"val").as_deref())),
        b"shd" => {
            ppr.shd_fill = attr_val(e, b"fill");
            ppr.shd_val = attr_val(e, b"val");
            ppr.shd_color = attr_val(e, b"color");
        }
        b"framePr" => {
            ppr.frame_pr = Some(DocxFramePr {
                drop_cap: attr_val(e, b"dropCap"),
                lines: attr_val(e, b"lines")
                    .as_deref()
                    .and_then(|v| v.parse().ok()),
                h_space: attr_val(e, b"hSpace")
                    .as_deref()
                    .and_then(|v| v.parse().ok()),
            });
        }
        _ => {}
    }
}

/// Parses a `w:pBdr` element. Called after Start("pBdr") is consumed.
fn parse_pbdr(reader: &mut Reader<&[u8]>) -> OoxmlResult<DocxPBdr> {
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
fn parse_tabs(reader: &mut Reader<&[u8]>, out: &mut Vec<DocxTab>) -> OoxmlResult<()> {
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
