// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 AppThere Loki contributors

//! XLSX importer.

use crate::constants::REL_OFFICE_DOCUMENT;
use crate::error::{OoxmlError, OoxmlWarning};
use crate::xml_util::{event_text, local_attr_val, local_name};
use loki_doc_model::io::macros::MacroPayload;
use loki_opc::{Package, PartName};
use loki_sheet_model::{DocumentMeta, Workbook, Worksheet};
use quick_xml::Reader;
use quick_xml::events::Event;
use std::io::{Read, Seek};

#[path = "import_styles.rs"]
mod styles;
#[path = "import_worksheet.rs"]
mod worksheet;

use styles::parse_styles;
use worksheet::parse_worksheet;

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
    /// Preserved VBA macro payload (`.xlsm`/`.xltm`), if present. Not
    /// executed in Phase 1; retained so a macro-enabled re-export does not
    /// strip it (spec §3).
    pub macros: Option<MacroPayload>,
}

/// Unit struct that implements XLSX spreadsheet import.
pub struct XlsxImport;

impl XlsxImport {
    /// Imports an XLSX file and returns the workbook.
    ///
    /// Discards warnings and any preserved macro payload; use
    /// [`XlsxImport::run`] to retrieve them.
    pub fn import(
        reader: impl Read + Seek,
        options: XlsxImportOptions,
    ) -> Result<Workbook, OoxmlError> {
        Self::run(reader, options).map(|r| r.workbook)
    }

    /// Imports an XLSX file, returning the workbook plus warnings and any
    /// preserved VBA macro payload.
    pub fn run(
        reader: impl Read + Seek,
        _options: XlsxImportOptions,
    ) -> Result<XlsxImportResult, OoxmlError> {
        let package = Package::open(reader)?;

        // 1. Locate the workbook (main document part)
        let doc_rel = rels_by_type(package.relationships(), REL_OFFICE_DOCUMENT)
            .next()
            .ok_or_else(|| OoxmlError::MissingPart {
                relationship_type: REL_OFFICE_DOCUMENT.to_owned(),
            })?
            .clone();

        let workbook_part_name = resolve_part_name("/", &doc_rel.target)?;
        let workbook_part =
            package
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
            if let Some(rel) = rels_by_type(
                rels,
                "http://schemas.openxmlformats.org/officeDocument/2006/relationships/sharedStrings",
            )
            .next()
            {
                let ss_part_name = resolve_part_name(workbook_part_name.as_str(), &rel.target)?;
                if let Some(part) = package.part(&ss_part_name) {
                    shared_strings = parse_shared_strings(&part.bytes)?;
                }
            }
        }

        // 5. Resolve styles if present
        let mut styles = Vec::new();
        if let Some(rels) = workbook_rels {
            if let Some(rel) = rels_by_type(
                rels,
                "http://schemas.openxmlformats.org/officeDocument/2006/relationships/styles",
            )
            .next()
            {
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
                    let sheet_part_name =
                        resolve_part_name(workbook_part_name.as_str(), &rel.target)?;
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

        // Preserve any VBA macro payload (spec §3, Phase 1).
        let macros = crate::vba::collect(&package, &workbook_part_name);

        Ok(XlsxImportResult {
            workbook: Workbook {
                meta: DocumentMeta::default(),
                sheets,
            },
            warnings: Vec::new(),
            macros,
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
                    if let (Some(name), Some(rel_id)) =
                        (local_attr_val(e, b"name"), local_attr_val(e, b"id"))
                    {
                        sheets.push(RawSheet { name, rel_id });
                    }
                }
            }
            Ok(Event::Eof) => break,
            Err(source) => {
                return Err(OoxmlError::Xml {
                    part: "xl/workbook.xml".to_owned(),
                    source,
                });
            }
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
            Ok(ref ev @ (Event::Text(_) | Event::GeneralRef(_))) => {
                if in_t {
                    current_string.push_str(&event_text(ev).unwrap_or_default());
                }
            }
            Ok(Event::Eof) => break,
            Err(source) => {
                return Err(OoxmlError::Xml {
                    part: "xl/sharedStrings.xml".to_owned(),
                    source,
                });
            }
            _ => {}
        }
        buf.clear();
    }
    Ok(strings)
}

// ── Column width conversion ──────────────────────────────────────────────────
//
// XLSX column width is in "character" units of the Normal-style font. Using the
// default Calibri 11 max-digit-width (7px) plus 5px cell padding, and 96 px/in
// vs 72 pt/in, the conversion is linear and exactly invertible by
// `crate::xlsx::export::pt_to_xlsx_char_width`.

/// Pixels per character (Calibri 11 max digit width).
pub(crate) const CHAR_WIDTH_PX: f64 = 7.0;
/// Fixed cell padding in pixels.
pub(crate) const CELL_PADDING_PX: f64 = 5.0;

/// Converts an XLSX character-unit column width to points.
pub(crate) fn xlsx_char_width_to_pt(width_chars: f64) -> f64 {
    let px = width_chars * CHAR_WIDTH_PX + CELL_PADDING_PX;
    px * 72.0 / 96.0
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
