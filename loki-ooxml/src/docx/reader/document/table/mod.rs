// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 AppThere Loki contributors

//! Parsers for `w:tbl`, `w:tr`, `w:tblPr`, and related table-level structures.

mod cell;

use quick_xml::Reader;
use quick_xml::events::Event;

use crate::docx::model::styles::{DocxTableModel, DocxTableRow, DocxTblPr, DocxTblWidth, DocxTrPr};
use crate::docx::reader::util::{attr_val, local_name};
use crate::error::{OoxmlError, OoxmlResult};

use cell::parse_table_cell;

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
