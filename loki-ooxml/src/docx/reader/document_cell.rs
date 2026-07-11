// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 AppThere Loki contributors

//! DOCX table-cell parsing: `w:tc` and its `w:tcPr` (borders, margins,
//! vertical alignment/merge, text direction). Split out of `document.rs`
//! (Phase 7.1); `parse_table_row` calls `parse_table_cell`, whose content
//! recurses through `super::{parse_paragraph, parse_table}`.

use quick_xml::{Reader, events::Event};

use crate::docx::model::document::DocxBodyChild;
use crate::docx::model::paragraph::DocxBorderEdge;
use crate::docx::model::styles::{
    DocxCellMargins, DocxTableCell, DocxTcBorders, DocxTcPr, DocxTextDirection, DocxVAlign,
};
use crate::docx::reader::util::{attr_val, local_name};
use crate::error::{OoxmlError, OoxmlResult};

use super::parse_paragraph;
use super::table::parse_table;

/// Parses a `w:tc` element. Called after Start("tc") is consumed.
pub(super) fn parse_table_cell(reader: &mut Reader<&[u8]>) -> OoxmlResult<DocxTableCell> {
    let mut cell = DocxTableCell::default();
    let mut buf = Vec::new();
    loop {
        match reader.read_event_into(&mut buf) {
            Ok(Event::Start(ref e)) => match local_name(e.local_name().as_ref()) {
                b"tcPr" => {
                    cell.tc_pr = Some(parse_tc_pr(reader)?);
                }
                b"p" => {
                    let para = parse_paragraph(reader)?;
                    cell.children.push(DocxBodyChild::Paragraph(para));
                }
                b"tbl" => {
                    // Nested table inside this cell (ECMA-376 §17.4.4).
                    let tbl = parse_table(reader)?;
                    cell.children.push(DocxBodyChild::Table(tbl));
                }
                _ => {}
            },
            Ok(Event::End(ref e)) if local_name(e.local_name().as_ref()) == b"tc" => {
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
    Ok(cell)
}

/// Parses a `w:tcPr` element. Called after Start("tcPr") is consumed.
fn parse_tc_pr(reader: &mut Reader<&[u8]>) -> OoxmlResult<DocxTcPr> {
    let mut tc_pr = DocxTcPr::default();
    let mut buf = Vec::new();
    loop {
        match reader.read_event_into(&mut buf) {
            Ok(Event::Empty(ref e) | Event::Start(ref e)) => {
                match local_name(e.local_name().as_ref()) {
                    b"gridSpan" => {
                        tc_pr.grid_span = attr_val(e, b"val").and_then(|v| v.parse().ok());
                    }
                    b"vMerge" => {
                        use crate::docx::model::styles::DocxVMerge;
                        tc_pr.v_merge =
                            Some(if attr_val(e, b"val").as_deref() == Some("restart") {
                                DocxVMerge::Restart
                            } else {
                                DocxVMerge::Continue
                            });
                    }
                    b"shd" => {
                        tc_pr.shd_fill = attr_val(e, b"fill");
                        tc_pr.shd_val = attr_val(e, b"val");
                        tc_pr.shd_color = attr_val(e, b"color");
                    }
                    b"tcBorders" => {
                        tc_pr.tc_borders = Some(parse_tc_borders(reader)?);
                        // parse_tc_borders consumes until </tcBorders>, so skip
                        // the fallthrough End event that would match here.
                        buf.clear();
                        continue;
                    }
                    b"tcMar" => {
                        tc_pr.tc_margins = Some(parse_tc_margins(reader)?);
                        buf.clear();
                        continue;
                    }
                    b"vAlign" => {
                        tc_pr.v_align = match attr_val(e, b"val").as_deref() {
                            Some("top") => Some(DocxVAlign::Top),
                            Some("center") => Some(DocxVAlign::Center),
                            Some("bottom") => Some(DocxVAlign::Bottom),
                            _ => None,
                        };
                    }
                    b"textDirection" => {
                        tc_pr.text_direction = match attr_val(e, b"val").as_deref() {
                            Some("lrTb") => Some(DocxTextDirection::LrTb),
                            Some("tbRl") => Some(DocxTextDirection::TbRl),
                            Some("tbLr") => Some(DocxTextDirection::TbLr),
                            Some("btLr") => Some(DocxTextDirection::BtLr),
                            _ => None,
                        };
                    }
                    _ => {}
                }
            }
            Ok(Event::End(ref e)) if local_name(e.local_name().as_ref()) == b"tcPr" => {
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
    Ok(tc_pr)
}

/// Parses a `w:tcBorders` element. Called after Start("tcBorders") is consumed.
fn parse_tc_borders(reader: &mut Reader<&[u8]>) -> OoxmlResult<DocxTcBorders> {
    let mut borders = DocxTcBorders::default();
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
                    b"top" => borders.top = Some(edge),
                    b"bottom" => borders.bottom = Some(edge),
                    b"left" | b"start" => borders.left = Some(edge),
                    b"right" | b"end" => borders.right = Some(edge),
                    _ => {}
                }
            }
            Ok(Event::End(ref e)) if local_name(e.local_name().as_ref()) == b"tcBorders" => {
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
    Ok(borders)
}

/// Parses a `w:tcMar` element. Called after Start("tcMar") is consumed.
/// Values are in twips (twentieths of a point); COMPAT(ooxml-dxa): divide by 20 for points.
fn parse_tc_margins(reader: &mut Reader<&[u8]>) -> OoxmlResult<DocxCellMargins> {
    let mut margins = DocxCellMargins::default();
    let mut buf = Vec::new();
    loop {
        match reader.read_event_into(&mut buf) {
            Ok(Event::Empty(ref e) | Event::Start(ref e)) => {
                let twips: Option<i32> = attr_val(e, b"w").and_then(|v| v.parse().ok());
                match local_name(e.local_name().as_ref()) {
                    b"top" => margins.top = twips,
                    b"bottom" => margins.bottom = twips,
                    b"left" | b"start" => margins.left = twips,
                    b"right" | b"end" => margins.right = twips,
                    _ => {}
                }
            }
            Ok(Event::End(ref e)) if local_name(e.local_name().as_ref()) == b"tcMar" => {
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
    Ok(margins)
}
