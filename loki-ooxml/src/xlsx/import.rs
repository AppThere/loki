// Copyright 2026 AppThere Loki contributors
// SPDX-License-Identifier: Apache-2.0

//! XLSX importer.

use std::collections::HashMap;
use std::io::{Read, Seek};
use loki_sheet_model::{Workbook, Worksheet, Cell, CellStyle, CellAlign, NumberFormat, DocumentMeta};
use loki_opc::{Package, PartName};
use crate::constants::REL_OFFICE_DOCUMENT;
use crate::error::{OoxmlError, OoxmlWarning};
use crate::xml_util::{local_name, local_attr_val};
use quick_xml::Reader;
use quick_xml::events::Event;

/// Options controlling XLSX import behaviour.
#[derive(Debug, Clone, Default)]
pub struct XlsxImportOptions {}

/// The result of a successful XLSX import.
#[derive(Debug)]
pub struct XlsxImportResult {
    /// The imported workbook model.
    pub workbook: Workbook,
    /// Non-fatal warnings.
    pub warnings: Vec<OoxmlWarning>,
}

/// Unit struct that implements XLSX spreadsheet import.
pub struct XlsxImport;

impl XlsxImport {
    /// Imports an XLSX file and returns the workbook.
    pub fn import(
        reader: impl Read + Seek,
        _options: XlsxImportOptions,
    ) -> Result<Workbook, OoxmlError> {
        let package = Package::open(reader)?;

        // 1. Locate the workbook (main document part)
        let doc_rel = rels_by_type(package.relationships(), REL_OFFICE_DOCUMENT)
            .next()
            .ok_or_else(|| OoxmlError::MissingPart {
                relationship_type: REL_OFFICE_DOCUMENT.to_owned(),
            })?
            .clone();

        let workbook_part_name = resolve_part_name("/", &doc_rel.target)?;
        let workbook_part = package
            .part(&workbook_part_name)
            .ok_or_else(|| OoxmlError::MissingPart {
                relationship_type: workbook_part_name.as_str().to_owned(),
            })?;

        // 2. Parse workbook to get sheets list
        let raw_sheets = parse_workbook_sheets(&workbook_part.bytes)?;

        // 3. Resolve workbook relationships
        let workbook_rels = package.part_relationships(&workbook_part_name);

        // 4. Resolve sharedStrings if present
        let mut shared_strings = Vec::new();
        if let Some(rels) = workbook_rels {
            if let Some(rel) = rels_by_type(rels, "http://schemas.openxmlformats.org/officeDocument/2006/relationships/sharedStrings").next() {
                let ss_part_name = resolve_part_name(workbook_part_name.as_str(), &rel.target)?;
                if let Some(part) = package.part(&ss_part_name) {
                    shared_strings = parse_shared_strings(&part.bytes)?;
                }
            }
        }

        // 5. Resolve styles if present
        let mut styles = Vec::new();
        if let Some(rels) = workbook_rels {
            if let Some(rel) = rels_by_type(rels, "http://schemas.openxmlformats.org/officeDocument/2006/relationships/styles").next() {
                let styles_part_name = resolve_part_name(workbook_part_name.as_str(), &rel.target)?;
                if let Some(part) = package.part(&styles_part_name) {
                    styles = parse_styles(&part.bytes)?;
                }
            }
        }

        // 6. Parse sheets
        let mut sheets = Vec::new();
        if let Some(rels) = workbook_rels {
            for rs in raw_sheets {
                if let Some(rel) = rels.iter().find(|r| r.id == rs.rel_id) {
                    let sheet_part_name = resolve_part_name(workbook_part_name.as_str(), &rel.target)?;
                    if let Some(part) = package.part(&sheet_part_name) {
                        let mut worksheet = parse_worksheet(&part.bytes, &shared_strings, &styles)?;
                        worksheet.name = rs.name;
                        sheets.push(worksheet);
                    }
                }
            }
        }

        if sheets.is_empty() {
            sheets.push(Worksheet::new("Sheet1"));
        }

        Ok(Workbook {
            meta: DocumentMeta::default(),
            sheets,
        })
    }
}

// ── XML Parsing Helpers ──────────────────────────────────────────────────────

struct RawSheet {
    name: String,
    rel_id: String,
}

fn parse_workbook_sheets(data: &[u8]) -> Result<Vec<RawSheet>, OoxmlError> {
    let mut reader = Reader::from_reader(data);
    reader.config_mut().trim_text(false);
    let mut buf = Vec::new();
    let mut sheets = Vec::new();

    loop {
        match reader.read_event_into(&mut buf) {
            Ok(Event::Start(ref e)) | Ok(Event::Empty(ref e)) => {
                if local_name(e) == b"sheet" {
                    if let (Some(name), Some(rel_id)) = (local_attr_val(e, b"name"), local_attr_val(e, b"id")) {
                        sheets.push(RawSheet { name, rel_id });
                    }
                }
            }
            Ok(Event::Eof) => break,
            Err(source) => return Err(OoxmlError::Xml { part: "xl/workbook.xml".to_owned(), source }),
            _ => {}
        }
        buf.clear();
    }
    Ok(sheets)
}

fn parse_shared_strings(data: &[u8]) -> Result<Vec<String>, OoxmlError> {
    let mut reader = Reader::from_reader(data);
    reader.config_mut().trim_text(false);
    let mut buf = Vec::new();
    let mut strings = Vec::new();
    let mut current_string = String::new();
    let mut in_t = false;

    loop {
        match reader.read_event_into(&mut buf) {
            Ok(Event::Start(ref e)) => {
                if local_name(e) == b"t" {
                    in_t = true;
                }
            }
            Ok(Event::End(ref e)) => {
                let name_bytes = e.local_name().into_inner();
                let name = if let Some(pos) = name_bytes.iter().position(|&b| b == b':') {
                    &name_bytes[pos + 1..]
                } else {
                    name_bytes
                };
                if name == b"t" {
                    in_t = false;
                } else if name == b"si" {
                    strings.push(std::mem::take(&mut current_string));
                }
            }
            Ok(Event::Text(ref e)) => {
                if in_t {
                    current_string.push_str(&e.unescape().unwrap_or_default());
                }
            }
            Ok(Event::Eof) => break,
            Err(source) => return Err(OoxmlError::Xml { part: "xl/sharedStrings.xml".to_owned(), source }),
            _ => {}
        }
        buf.clear();
    }
    Ok(strings)
}

fn parse_styles(data: &[u8]) -> Result<Vec<CellStyle>, OoxmlError> {
    let mut reader = Reader::from_reader(data);
    reader.config_mut().trim_text(false);
    let mut buf = Vec::new();
    
    let mut custom_num_formats = HashMap::new();
    let mut fonts = Vec::new();
    let mut cell_xfs = Vec::new();
    
    let mut in_cell_xfs = false;
    let mut in_font = false;
    let mut current_font = CellStyle::default();

    macro_rules! handle_start {
        ($e:expr) => {{
            let e = $e;
            match local_name(e) {
                b"numFmt" => {
                    if let (Some(id_str), Some(code)) = (local_attr_val(e, b"numFmtId"), local_attr_val(e, b"formatCode")) {
                        if let Ok(id) = id_str.parse::<u32>() {
                            let code_lower = code.to_lowercase();
                            let fmt = if code_lower.contains('%') {
                                NumberFormat::Percent
                            } else if code_lower.contains('$') || code_lower.contains('£') || code_lower.contains('€') || code_lower.contains('¥') {
                                NumberFormat::Currency
                            } else {
                                NumberFormat::General
                            };
                            custom_num_formats.insert(id, fmt);
                        }
                    }
                }
                b"font" => {
                    current_font = CellStyle::default();
                    in_font = true;
                }
                b"b" => {
                    if in_font {
                        current_font.bold = true;
                    }
                }
                b"i" => {
                    if in_font {
                        current_font.italic = true;
                    }
                }
                b"u" => {
                    if in_font {
                        current_font.underline = true;
                    }
                }
                b"cellXfs" => {
                    in_cell_xfs = true;
                }
                b"xf" => {
                    if in_cell_xfs {
                        let font_id = local_attr_val(e, b"fontId")
                            .and_then(|s| s.parse::<usize>().ok())
                            .unwrap_or(0);
                        let num_fmt_id = local_attr_val(e, b"numFmtId")
                            .and_then(|s| s.parse::<u32>().ok())
                            .unwrap_or(0);
                        
                        let mut style = CellStyle::default();
                        style.bold = fonts.get(font_id).map(|f: &CellStyle| f.bold).unwrap_or(false);
                        style.italic = fonts.get(font_id).map(|f: &CellStyle| f.italic).unwrap_or(false);
                        style.underline = fonts.get(font_id).map(|f: &CellStyle| f.underline).unwrap_or(false);
                        
                        let num_fmt = match num_fmt_id {
                            9 | 10 => NumberFormat::Percent,
                            5 | 6 | 7 | 8 | 44 => NumberFormat::Currency,
                            id => custom_num_formats.get(&id).cloned().unwrap_or(NumberFormat::General),
                        };
                        style.num_format = num_fmt;
                        
                        cell_xfs.push(style);
                    }
                }
                b"alignment" => {
                    if in_cell_xfs {
                        if let Some(last_xf) = cell_xfs.last_mut() {
                            if let Some(horiz) = local_attr_val(e, b"horizontal") {
                                last_xf.align = match horiz.as_str() {
                                    "center" => CellAlign::Center,
                                    "right" => CellAlign::Right,
                                    _ => CellAlign::Left,
                                };
                            }
                        }
                    }
                }
                _ => {}
            }
        }};
    }

    macro_rules! handle_end {
        ($name:expr) => {{
            match $name {
                b"font" => {
                    fonts.push(std::mem::take(&mut current_font));
                    in_font = false;
                }
                b"cellXfs" => {
                    in_cell_xfs = false;
                }
                _ => {}
            }
        }};
    }

    loop {
        match reader.read_event_into(&mut buf) {
            Ok(Event::Start(ref e)) => handle_start!(e),
            Ok(Event::End(ref e)) => {
                let name_bytes = e.local_name().into_inner();
                let name = if let Some(pos) = name_bytes.iter().position(|&b| b == b':') {
                    &name_bytes[pos + 1..]
                } else {
                    name_bytes
                };
                handle_end!(name);
            }
            Ok(Event::Empty(ref e)) => {
                handle_start!(e);
                let name_bytes = e.local_name().into_inner();
                let name = if let Some(pos) = name_bytes.iter().position(|&b| b == b':') {
                    &name_bytes[pos + 1..]
                } else {
                    name_bytes
                };
                handle_end!(name);
            }
            Ok(Event::Eof) => break,
            Err(source) => return Err(OoxmlError::Xml { part: "xl/styles.xml".to_owned(), source }),
            _ => {}
        }
        buf.clear();
    }
    
    Ok(cell_xfs)
}

fn parse_worksheet(
    data: &[u8],
    shared_strings: &[String],
    styles: &[CellStyle],
) -> Result<Worksheet, OoxmlError> {
    let mut worksheet = Worksheet::new("Sheet");
    let mut reader = Reader::from_reader(data);
    reader.config_mut().trim_text(false);
    let mut buf = Vec::new();
    
    let mut current_ref = None;
    let mut current_type = None;
    let mut current_style_idx = None;
    let mut current_formula = None;
    let mut current_value = String::new();
    
    let mut in_f = false;
    let mut in_v = false;
    let mut in_is_t = false;

    macro_rules! handle_start {
        ($e:expr) => {{
            let e = $e;
            match local_name(e) {
                b"c" => {
                    current_ref = local_attr_val(e, b"r");
                    current_type = local_attr_val(e, b"t");
                    current_style_idx = local_attr_val(e, b"s").and_then(|s| s.parse::<usize>().ok());
                    current_formula = None;
                    current_value.clear();
                }
                b"f" => {
                    in_f = true;
                }
                b"v" => {
                    in_v = true;
                }
                b"t" => {
                    in_is_t = true;
                }
                _ => {}
            }
        }};
    }

    macro_rules! handle_end {
        ($name:expr) => {{
            match $name {
                b"c" => {
                    if let Some(r_str) = &current_ref {
                        if let Some((row, col)) = cell_ref_to_coord(r_str) {
                            let final_value = if current_type.as_deref() == Some("s") {
                                if let Ok(idx) = current_value.parse::<usize>() {
                                    shared_strings.get(idx).cloned().unwrap_or_default()
                                } else {
                                    current_value.clone()
                                }
                            } else {
                                current_value.clone()
                            };
                            
                            let style = current_style_idx.and_then(|idx| styles.get(idx).cloned());
                            
                            worksheet.cells.insert((row, col), Cell {
                                value: final_value,
                                formula: current_formula.clone(),
                                style,
                            });
                        }
                    }
                    current_ref = None;
                    current_type = None;
                    current_style_idx = None;
                }
                b"f" => {
                    in_f = false;
                }
                b"v" => {
                    in_v = false;
                }
                b"t" => {
                    in_is_t = false;
                }
                _ => {}
            }
        }};
    }

    loop {
        match reader.read_event_into(&mut buf) {
            Ok(Event::Start(ref e)) => handle_start!(e),
            Ok(Event::End(ref e)) => {
                let name_bytes = e.local_name().into_inner();
                let name = if let Some(pos) = name_bytes.iter().position(|&b| b == b':') {
                    &name_bytes[pos + 1..]
                } else {
                    name_bytes
                };
                handle_end!(name);
            }
            Ok(Event::Empty(ref e)) => {
                handle_start!(e);
                let name_bytes = e.local_name().into_inner();
                let name = if let Some(pos) = name_bytes.iter().position(|&b| b == b':') {
                    &name_bytes[pos + 1..]
                } else {
                    name_bytes
                };
                handle_end!(name);
            }
            Ok(Event::Text(ref e)) => {
                let text = e.unescape().unwrap_or_default().into_owned();
                if in_f {
                    current_formula = Some(text);
                } else if in_v || in_is_t {
                    current_value.push_str(&text);
                }
            }
            Ok(Event::Eof) => break,
            Err(source) => return Err(OoxmlError::Xml { part: "sheet.xml".to_owned(), source }),
            _ => {}
        }
        buf.clear();
    }
    
    Ok(worksheet)
}

// ── Part Name Resolution Helpers ─────────────────────────────────────────────

fn resolve_part_name(base: &str, target: &str) -> Result<PartName, OoxmlError> {
    if target.starts_with('/') {
        return PartName::new(target).map_err(OoxmlError::Opc);
    }
    let dir = base.rfind('/').map_or("/", |i| &base[..=i]);
    PartName::new(format!("{dir}{target}")).map_err(OoxmlError::Opc)
}

fn rels_by_type<'a>(
    rels: &'a loki_opc::RelationshipSet,
    transitional_type: &str,
) -> impl Iterator<Item = &'a loki_opc::Relationship> {
    let strict_type = transitional_type.replace(
        "http://schemas.openxmlformats.org/officeDocument/2006/relationships/",
        "http://purl.oclc.org/ooxml/officeDocument/relationships/",
    );
    let strict_owned = strict_type;
    let trans_owned = transitional_type.to_owned();
    rels.iter()
        .filter(move |r| r.rel_type == trans_owned || r.rel_type == strict_owned)
}

// ── Coordinate Conversion Helpers ──────────────────────────────────────────

fn cell_ref_to_coord(cell_ref: &str) -> Option<(u32, u32)> {
    let mut chars = cell_ref.chars().peekable();
    let mut col_str = String::new();
    while let Some(&c) = chars.peek() {
        if c.is_ascii_alphabetic() {
            col_str.push(c.to_ascii_uppercase());
            chars.next();
        } else {
            break;
        }
    }
    let row_str: String = chars.collect();
    if col_str.is_empty() || row_str.is_empty() {
        return None;
    }
    let row = row_str.parse::<u32>().ok()?.checked_sub(1)?;
    
    let mut col: u32 = 0;
    for c in col_str.chars() {
        col = col.checked_mul(26)?.checked_add((c as u32) - ('A' as u32) + 1)?;
    }
    col = col.checked_sub(1)?;
    Some((row, col))
}
