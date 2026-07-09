// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 AppThere Loki contributors

//! Table parsing for the ODT `content.xml` reader: `table:table` and its
//! header-rows / rows / cells (cell content recurses back through
//! `super::{read_paragraph, read_list}` and nested `read_table`). Split out of
//! `document.rs` (Phase 7.1). ODF 1.3 §9.1.

// `drop(ref_binding)` is a deliberate NLL-boundary hint (see `document.rs`).
#![allow(dropping_references)]

use quick_xml::Reader;
use quick_xml::events::{BytesStart, Event};

use crate::error::{OdfError, OdfResult};
use crate::odt::model::document::OdfBodyChild;
use crate::odt::model::tables::{OdfTable, OdfTableCell, OdfTableColDef, OdfTableRow};
use crate::xml_util::local_attr_val;

use super::{read_list, read_paragraph, skip_element};

/// Parse a `table:table` element. ODF 1.3 §9.1.
///
/// Called after consuming the `Start` event for `table:table`.
pub(crate) fn read_table(reader: &mut Reader<&[u8]>, tag: &BytesStart<'_>) -> OdfResult<OdfTable> {
    let name = local_attr_val(tag, b"name");
    let style_name = local_attr_val(tag, b"style-name");
    let mut col_defs: Vec<OdfTableColDef> = Vec::new();
    let mut rows: Vec<OdfTableRow> = Vec::new();
    let mut buf = Vec::new();
    loop {
        buf.clear();
        match reader.read_event_into(&mut buf) {
            Ok(Event::Start(ref e)) => {
                let local = e.local_name().into_inner().to_vec();
                match local.as_slice() {
                    b"table-column" => {
                        let col_style = local_attr_val(e, b"style-name");
                        let columns_repeated: u32 = local_attr_val(e, b"number-columns-repeated")
                            .and_then(|s| s.parse().ok())
                            .unwrap_or(1);
                        drop(e);
                        skip_element(reader)?;
                        col_defs.push(OdfTableColDef {
                            style_name: col_style,
                            columns_repeated,
                        });
                    }
                    b"table-header-rows" => {
                        drop(e);
                        read_table_header_rows(reader, &mut rows)?;
                    }
                    b"table-row" => {
                        let row = read_table_row(reader, e, false)?;
                        rows.push(row);
                    }
                    _ => {
                        drop(e);
                        skip_element(reader)?;
                    }
                }
            }
            Ok(Event::Empty(ref e)) => {
                let local = e.local_name().into_inner();
                if local == b"table-column" {
                    let col_style = local_attr_val(e, b"style-name");
                    let columns_repeated: u32 = local_attr_val(e, b"number-columns-repeated")
                        .and_then(|s| s.parse().ok())
                        .unwrap_or(1);
                    col_defs.push(OdfTableColDef {
                        style_name: col_style,
                        columns_repeated,
                    });
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
    Ok(OdfTable {
        name,
        style_name,
        col_defs,
        rows,
    })
}

/// Read rows inside `table:table-header-rows`, marking each as `is_header`.
fn read_table_header_rows(
    reader: &mut Reader<&[u8]>,
    rows: &mut Vec<OdfTableRow>,
) -> OdfResult<()> {
    let mut buf = Vec::new();
    loop {
        buf.clear();
        match reader.read_event_into(&mut buf) {
            Ok(Event::Start(ref e)) => {
                if e.local_name().into_inner() == b"table-row" {
                    let row = read_table_row(reader, e, true)?;
                    rows.push(row);
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

/// Parse a `table:table-row` element. ODF 1.3 §9.3.
fn read_table_row(
    reader: &mut Reader<&[u8]>,
    tag: &BytesStart<'_>,
    _is_header: bool,
) -> OdfResult<OdfTableRow> {
    let style_name = local_attr_val(tag, b"style-name");
    let mut cells: Vec<OdfTableCell> = Vec::new();
    let mut buf = Vec::new();
    loop {
        buf.clear();
        match reader.read_event_into(&mut buf) {
            Ok(Event::Start(ref e)) => {
                let local = e.local_name().into_inner().to_vec();
                match local.as_slice() {
                    b"table-cell" => {
                        let cell = read_table_cell(reader, e, false)?;
                        cells.push(cell);
                    }
                    b"covered-table-cell" => {
                        drop(e);
                        skip_element(reader)?;
                        cells.push(OdfTableCell {
                            is_covered: true,
                            col_span: 1,
                            row_span: 1,
                            style_name: None,
                            value_type: None,
                            content: vec![],
                        });
                    }
                    _ => {
                        drop(e);
                        skip_element(reader)?;
                    }
                }
            }
            Ok(Event::Empty(ref e)) => {
                let local = e.local_name().into_inner();
                match local {
                    b"table-cell" => {
                        let style_name = local_attr_val(e, b"style-name");
                        let col_span: u32 = local_attr_val(e, b"number-columns-spanned")
                            .and_then(|s| s.parse().ok())
                            .unwrap_or(1);
                        let row_span: u32 = local_attr_val(e, b"number-rows-spanned")
                            .and_then(|s| s.parse().ok())
                            .unwrap_or(1);
                        let value_type = local_attr_val(e, b"value-type");
                        cells.push(OdfTableCell {
                            style_name,
                            col_span,
                            row_span,
                            is_covered: false,
                            value_type,
                            content: vec![],
                        });
                    }
                    b"covered-table-cell" => {
                        cells.push(OdfTableCell {
                            is_covered: true,
                            col_span: 1,
                            row_span: 1,
                            style_name: None,
                            value_type: None,
                            content: vec![],
                        });
                    }
                    _ => {}
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
    Ok(OdfTableRow { style_name, cells })
}

/// Parse a `table:table-cell` element. ODF 1.3 §9.4.
fn read_table_cell(
    reader: &mut Reader<&[u8]>,
    tag: &BytesStart<'_>,
    is_covered: bool,
) -> OdfResult<OdfTableCell> {
    let style_name = local_attr_val(tag, b"style-name");
    let col_span: u32 = local_attr_val(tag, b"number-columns-spanned")
        .and_then(|s| s.parse().ok())
        .unwrap_or(1);
    let row_span: u32 = local_attr_val(tag, b"number-rows-spanned")
        .and_then(|s| s.parse().ok())
        .unwrap_or(1);
    let value_type = local_attr_val(tag, b"value-type");
    // Ordered block content so paragraphs, lists, and nested `table:table`
    // children keep their document order (ODF 1.3 §9.4). A nested table recurses
    // through `read_table`, mirroring the body-level dispatch.
    let mut content: Vec<OdfBodyChild> = Vec::new();
    let mut buf = Vec::new();
    loop {
        buf.clear();
        match reader.read_event_into(&mut buf) {
            Ok(Event::Start(ref e)) => {
                let local = e.local_name().into_inner().to_vec();
                match local.as_slice() {
                    b"p" => {
                        let para = read_paragraph(reader, e)?;
                        content.push(OdfBodyChild::Paragraph(para));
                    }
                    b"h" => {
                        let para = read_paragraph(reader, e)?;
                        content.push(OdfBodyChild::Heading(para));
                    }
                    b"list" => {
                        let list = read_list(reader, e, None, 0)?;
                        content.push(OdfBodyChild::List(list));
                    }
                    b"table" => {
                        let table = read_table(reader, e)?;
                        content.push(OdfBodyChild::Table(table));
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
    Ok(OdfTableCell {
        style_name,
        col_span,
        row_span,
        is_covered,
        value_type,
        content,
    })
}
