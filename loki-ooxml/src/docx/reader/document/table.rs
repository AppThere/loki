// SPDX-License-Identifier: Apache-2.0
// Copyright (C) 2025 AppThere contributors

//! Parsers for `w:tbl`, `w:tr`, `w:tc`, and related table properties.

use quick_xml::Reader;
use quick_xml::events::Event;

use crate::docx::model::paragraph::DocxBorderEdge;
use crate::docx::model::styles::{
    DocxCellMargins, DocxTableCell, DocxTableModel, DocxTableRow, DocxTblPr, DocxTblWidth,
    DocxTcBorders, DocxTcPr, DocxTextDirection, DocxTrPr, DocxVAlign,
};
use crate::docx::reader::util::{attr_val, local_name};
use crate::error::{OoxmlError, OoxmlResult};

use super::para::parse_paragraph;

/// Parses a `w:tbl` element. Called after Start("tbl") is consumed.
pub(super) fn parse_table(reader: &mut Reader<&[u8]>) -> OoxmlResult<DocxTableModel> {
    let mut tbl = DocxTableModel::default();
    let mut buf = Vec::new();
    loop {
        match reader.read_event_into(&mut buf) {
            Ok(Event::Start(ref e)) => {
                match local_name(e.local_name().as_ref()) {
                    b"tr" => {
                        let row = parse_table_row(reader)?;
                        tbl.rows.push(row);
                    }
                    b"tblPr" => {
                        tbl.tbl_pr = Some(parse_tbl_pr(reader)?);
                    }
                    _ => {}
                }
                // tblGrid and gridCol: handled via Empty event below
            }
            Ok(Event::Empty(ref e)) if local_name(e.local_name().as_ref()) == b"gridCol" => {
                let w: i32 = attr_val(e, b"w").and_then(|v| v.parse().ok()).unwrap_or(0);
                tbl.col_widths.push(w);
            }
            Ok(Event::End(ref e)) if local_name(e.local_name().as_ref()) == b"tbl" => {
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
    Ok(tbl)
}

/// Parses a `w:tblPr` element. Called after Start("tblPr") is consumed.
fn parse_tbl_pr(reader: &mut Reader<&[u8]>) -> OoxmlResult<DocxTblPr> {
    let mut pr = DocxTblPr::default();
    let mut buf = Vec::new();
    loop {
        match reader.read_event_into(&mut buf) {
            Ok(Event::Empty(ref e)) => match local_name(e.local_name().as_ref()) {
                b"tblW" => {
                    if let (Some(w), Some(w_type)) = (
                        attr_val(e, b"w").and_then(|v| v.parse::<i32>().ok()),
                        attr_val(e, b"type"),
                    ) {
                        pr.width = Some(DocxTblWidth { w, w_type });
                    }
                }
                b"tblStyle" => {
                    pr.style_id = attr_val(e, b"val");
                }
                _ => {}
            },
            Ok(Event::End(ref e)) if local_name(e.local_name().as_ref()) == b"tblPr" => {
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
    Ok(pr)
}

/// Parses a `w:tr` element. Called after Start("tr") is consumed.
fn parse_table_row(reader: &mut Reader<&[u8]>) -> OoxmlResult<DocxTableRow> {
    let mut row = DocxTableRow::default();
    let mut buf = Vec::new();
    loop {
        match reader.read_event_into(&mut buf) {
            Ok(Event::Start(ref e)) => match local_name(e.local_name().as_ref()) {
                b"tc" => {
                    let cell = parse_table_cell(reader)?;
                    row.cells.push(cell);
                }
                b"trPr" => {
                    let mut tr_pr = DocxTrPr::default();
                    let mut tbuf = Vec::new();
                    loop {
                        match reader.read_event_into(&mut tbuf) {
                            Ok(Event::Empty(ref te))
                                if local_name(te.local_name().as_ref()) == b"tblHeader" =>
                            {
                                tr_pr.is_header = true;
                            }
                            Ok(Event::End(ref te))
                                if local_name(te.local_name().as_ref()) == b"trPr" =>
                            {
                                break;
                            }
                            Ok(Event::Eof) | Err(_) => break,
                            _ => {}
                        }
                        tbuf.clear();
                    }
                    row.tr_pr = Some(tr_pr);
                }
                _ => {}
            },
            Ok(Event::End(ref e)) if local_name(e.local_name().as_ref()) == b"tr" => {
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
    Ok(row)
}

/// Parses a `w:tc` element. Called after Start("tc") is consumed.
fn parse_table_cell(reader: &mut Reader<&[u8]>) -> OoxmlResult<DocxTableCell> {
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
                    cell.paragraphs.push(para);
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
