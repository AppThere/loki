// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 AppThere Loki contributors

//! XLSX importer.

mod shared_strings;
mod styles;
mod util;
mod workbook;
mod worksheet;

use crate::constants::REL_OFFICE_DOCUMENT;
use crate::error::{OoxmlError, OoxmlWarning};
use loki_opc::Package;
use loki_sheet_model::{DocumentMeta, Workbook, Worksheet};
use std::io::{Read, Seek};

use self::shared_strings::parse_shared_strings;
use self::styles::parse_styles;
use self::util::{rels_by_type, resolve_part_name};
use self::workbook::parse_workbook_sheets;
use self::worksheet::parse_worksheet;

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

        Ok(Workbook {
            meta: DocumentMeta::default(),
            sheets,
        })
    }
}
