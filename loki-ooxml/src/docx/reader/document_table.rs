// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 AppThere Loki contributors

//! DOCX table parsing: `w:tbl`, its `w:tblPr` (+ `w:tblLook`) and rows. Row
//! cells are parsed by `super::cell`; a cell's nested table recurses back
//! through `parse_table`. Split out of `document.rs` (Phase 7.1).

use quick_xml::{Reader, events::Event};

use crate::docx::model::styles::{
    DocxTableModel, DocxTableRow, DocxTblLook, DocxTblPr, DocxTblWidth, DocxTrPr,
};
use crate::docx::reader::util::{attr_val, local_name};
use crate::error::{OoxmlError, OoxmlResult};

use super::cell;

/// Maximum table/content-control nesting depth accepted from a file. Nested
/// tables recurse (`parse_table` → row → cell → `parse_table`), so without a
/// cap a crafted document can exhaust the stack (audit-2026-06 S-1b).
///
/// A single **table** level costs three stack frames (`parse_table`,
/// `parse_table_row`, `parse_table_cell`), so a depth of 100 recurses ~300
/// frames — enough to overflow a 2-`MiB` worker-thread stack (a real denial of
/// service when a server parses an uploaded document off the main thread). The
/// cap is chosen to reject deep nesting *before* that point: 50 levels stay
/// comfortably within a 2-`MiB` stack while still being ~10× the deepest real
/// documents (which rarely exceed ~5). The content-control (`w:sdt`) parser
/// shares this budget.
pub(super) const MAX_NESTING_DEPTH: usize = 50;

/// Parses a `w:tbl` element. Called after Start("tbl") is consumed. `depth`
/// counts enclosing tables/content controls; the top-level caller passes 0.
pub(super) fn parse_table(reader: &mut Reader<&[u8]>, depth: usize) -> OoxmlResult<DocxTableModel> {
    if depth > MAX_NESTING_DEPTH {
        return Err(OoxmlError::NestingTooDeep {
            limit: MAX_NESTING_DEPTH,
        });
    }
    let mut tbl = DocxTableModel::default();
    let mut buf = Vec::new();
    loop {
        match reader.read_event_into(&mut buf) {
            Ok(Event::Start(ref e)) => {
                match local_name(e.local_name().as_ref()) {
                    b"tr" => {
                        let row = parse_table_row(reader, depth)?;
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
                b"tblLayout" => {
                    pr.layout = attr_val(e, b"type");
                }
                b"tblLook" => {
                    pr.tbl_look = Some(parse_tbl_look(e));
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

/// Parses a `w:tblLook` element (ECMA-376 §17.4.56). Prefers the explicit
/// boolean attributes (`w:firstRow`, …); falls back to the legacy `w:val`
/// hex bitmask. `noHBand`/`noVBand` are inverted into positive banding flags.
pub(super) fn parse_tbl_look(e: &quick_xml::events::BytesStart<'_>) -> DocxTblLook {
    let flag = |name: &[u8]| attr_val(e, name).map(|v| v == "1" || v == "true");
    let val = attr_val(e, b"val").and_then(|v| u32::from_str_radix(&v, 16).ok());
    let bit = |mask: u32| val.map(|v| v & mask != 0);
    // Banding is on unless the corresponding `no*Band` bit/flag is set.
    let banding =
        |flag_name: &[u8], mask: u32| flag(flag_name).or_else(|| bit(mask)).is_some_and(|no| !no);
    DocxTblLook {
        first_row: flag(b"firstRow").or_else(|| bit(0x0020)).unwrap_or(false),
        last_row: flag(b"lastRow").or_else(|| bit(0x0040)).unwrap_or(false),
        first_column: flag(b"firstColumn")
            .or_else(|| bit(0x0080))
            .unwrap_or(false),
        last_column: flag(b"lastColumn").or_else(|| bit(0x0100)).unwrap_or(false),
        h_band: banding(b"noHBand", 0x0200),
        v_band: banding(b"noVBand", 0x0400),
    }
}

/// Parses a `w:tr` element. Called after Start("tr") is consumed.
fn parse_table_row(reader: &mut Reader<&[u8]>, depth: usize) -> OoxmlResult<DocxTableRow> {
    let mut row = DocxTableRow::default();
    let mut buf = Vec::new();
    loop {
        match reader.read_event_into(&mut buf) {
            Ok(Event::Start(ref e)) => match local_name(e.local_name().as_ref()) {
                b"tc" => {
                    let cell = cell::parse_table_cell(reader, depth)?;
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
