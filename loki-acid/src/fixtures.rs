// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 AppThere Loki contributors

//! The acid fixture documents and their import dispatch.
//!
//! Fixtures are embedded with `include_bytes!` so the harness is self-contained
//! and reproducible regardless of the working directory.

use std::io::Cursor;

use loki_doc_model::Document;
use loki_doc_model::io::DocumentImport;
use loki_odf::{OdsImport, OdsImportOptions, OdtImport, OdtImportOptions};
use loki_ooxml::{DocxImport, DocxImportOptions, XlsxImport, XlsxImportOptions};
use loki_sheet_model::workbook::Workbook;

use crate::catalog::Format;

/// One acid fixture file.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Fixture {
    /// `acid_docx.docx` — word-processing.
    Docx,
    /// `acid_odt.odt` — OpenDocument Text.
    Odt,
    /// `acid_xlsx.xlsx` — spreadsheet.
    Xlsx,
    /// `acid_ods.ods` — OpenDocument Spreadsheet.
    Ods,
    /// `acid_odp.odp` — OpenDocument Presentation (no importer yet).
    Odp,
    /// `acid_odg.odg` — OpenDocument Graphics (no importer yet).
    Odg,
}

/// The successful product of importing a fixture.
pub enum Imported {
    /// A word-processing document (paginatable).
    Document(Box<Document>),
    /// A spreadsheet workbook.
    Workbook(Box<Workbook>),
}

impl Fixture {
    /// All supplied fixtures (PPTX is not yet supplied).
    #[must_use]
    pub fn all() -> &'static [Fixture] {
        &[
            Fixture::Docx,
            Fixture::Odt,
            Fixture::Xlsx,
            Fixture::Ods,
            Fixture::Odp,
            Fixture::Odg,
        ]
    }

    /// The format this fixture belongs to.
    #[must_use]
    pub fn format(self) -> Format {
        match self {
            Fixture::Docx => Format::Docx,
            Fixture::Odt => Format::Odt,
            Fixture::Xlsx => Format::Xlsx,
            Fixture::Ods => Format::Ods,
            Fixture::Odp => Format::Odp,
            Fixture::Odg => Format::Odg,
        }
    }

    /// The fixture's asset file name.
    #[must_use]
    pub fn asset_name(self) -> &'static str {
        match self {
            Fixture::Docx => "acid_docx.docx",
            Fixture::Odt => "acid_odt.odt",
            Fixture::Xlsx => "acid_xlsx.xlsx",
            Fixture::Ods => "acid_ods.ods",
            Fixture::Odp => "acid_odp.odp",
            Fixture::Odg => "acid_odg.odg",
        }
    }

    /// The embedded fixture bytes.
    #[must_use]
    pub fn bytes(self) -> &'static [u8] {
        match self {
            Fixture::Docx => include_bytes!("../assets/acid_docx.docx"),
            Fixture::Odt => include_bytes!("../assets/acid_odt.odt"),
            Fixture::Xlsx => include_bytes!("../assets/acid_xlsx.xlsx"),
            Fixture::Ods => include_bytes!("../assets/acid_ods.ods"),
            Fixture::Odp => include_bytes!("../assets/acid_odp.odp"),
            Fixture::Odg => include_bytes!("../assets/acid_odg.odg"),
        }
    }

    /// `true` when an importer exists for this fixture's format.
    #[must_use]
    pub fn has_importer(self) -> bool {
        matches!(
            self,
            Fixture::Docx | Fixture::Odt | Fixture::Xlsx | Fixture::Ods
        )
    }

    /// Imports the fixture into the abstract model.
    ///
    /// Returns `Err` describing the failure, including the documented
    /// "no importer yet" case for ODP/ODG.
    pub fn import(self) -> Result<Imported, String> {
        let cursor = Cursor::new(self.bytes());
        match self {
            Fixture::Docx => DocxImport::import(cursor, DocxImportOptions::default())
                .map(|d| Imported::Document(Box::new(d)))
                .map_err(|e| e.to_string()),
            Fixture::Odt => OdtImport::import(cursor, OdtImportOptions::default())
                .map(|d| Imported::Document(Box::new(d)))
                .map_err(|e| e.to_string()),
            Fixture::Xlsx => XlsxImport::import(cursor, XlsxImportOptions::default())
                .map(|w| Imported::Workbook(Box::new(w)))
                .map_err(|e| e.to_string()),
            Fixture::Ods => OdsImport::import(cursor, OdsImportOptions::default())
                .map(|w| Imported::Workbook(Box::new(w)))
                .map_err(|e| e.to_string()),
            Fixture::Odp | Fixture::Odg => Err("no importer yet for this ODF format".to_string()),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_fixture_has_nonempty_bytes() {
        for &f in Fixture::all() {
            assert!(!f.bytes().is_empty(), "{} is empty", f.asset_name());
        }
    }

    #[test]
    fn fixture_formats_are_distinct() {
        let mut formats: Vec<Format> = Fixture::all().iter().map(|f| f.format()).collect();
        formats.sort_by_key(|f| format!("{f:?}"));
        formats.dedup_by_key(|f| format!("{f:?}"));
        assert_eq!(formats.len(), Fixture::all().len());
    }
}
