// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 AppThere Loki contributors

//! The acid fixture documents and their import dispatch.
//!
//! Fixtures are embedded with `include_bytes!` so the harness is self-contained
//! and reproducible regardless of the working directory.

use std::borrow::Cow;
use std::io::Cursor;

use appthere_conformance::corpus::{self, Axis, FixtureMeta, Severity};
use loki_doc_model::Document;
use loki_doc_model::io::DocumentImport;
use loki_odf::{OdsImport, OdsImportOptions, OdtImport, OdtImportOptions};
use loki_ooxml::pptx::import::{PptxImport, PptxImportOptions};
use loki_ooxml::{DocxImport, DocxImportOptions, XlsxImport, XlsxImportOptions};
use loki_presentation_model::Presentation;
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
    /// `acid_pptx.pptx` — presentation.
    Pptx,
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
    /// A slide presentation.
    Presentation(Box<Presentation>),
}

impl Fixture {
    /// All supplied fixtures (PPTX is not yet supplied).
    #[must_use]
    pub fn all() -> &'static [Fixture] {
        &[
            Fixture::Docx,
            Fixture::Odt,
            Fixture::Xlsx,
            Fixture::Pptx,
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
            Fixture::Pptx => Format::Pptx,
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
            Fixture::Pptx => "acid_pptx.pptx",
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
            Fixture::Pptx => include_bytes!("../assets/acid_pptx.pptx"),
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
            Fixture::Docx | Fixture::Odt | Fixture::Xlsx | Fixture::Pptx | Fixture::Ods
        )
    }

    /// Imports the fixture into the abstract model.
    ///
    /// Returns `Err` describing the failure, including the documented
    /// "no importer yet" case for ODP/ODG.
    pub fn import(self) -> Result<Imported, String> {
        import_bytes(self, self.bytes())
    }
}

/// The acid corpus's [`corpus::Fixture`] impl: each embedded acid document is
/// one composite fixture. Round-trip/schema axes apply only where an importer
/// exists; the visual axis always applies (the corpus exists to be rendered).
impl corpus::Fixture for Fixture {
    fn meta(&self) -> FixtureMeta {
        const WITH_IMPORTER: &[Axis] = &[Axis::Schema, Axis::RoundTrip, Axis::Visual];
        const VISUAL_ONLY: &[Axis] = &[Axis::Visual];
        FixtureMeta {
            id: crate::golden::fixture_stem(*self),
            format: self.format(),
            feature: "ACID composite (all catalogued constructs for this format)",
            severity: Severity::P0,
            axes: if self.has_importer() {
                WITH_IMPORTER
            } else {
                VISUAL_ONLY
            },
            // Goldens are produced manually from the canonical authority; no
            // pinned version is recorded yet (see README golden workflow).
            reference: None,
            tolerance_override: None,
        }
    }

    fn bytes(&self) -> Cow<'_, [u8]> {
        Cow::Borrowed(Fixture::bytes(*self))
    }
}

/// The acid corpus's [`corpus::Consumer`]: dispatches each fixture to the
/// suite importer for its format. The corpus is import-fidelity only, so
/// `export` reports that limitation rather than pretending.
pub struct AcidConsumer;

impl corpus::Consumer for AcidConsumer {
    type Model = Imported;

    fn import(&self, fixture: &dyn corpus::Fixture) -> Result<Imported, String> {
        let meta = fixture.meta();
        let all = Fixture::all()
            .iter()
            .find(|f| f.format() == meta.format)
            .ok_or_else(|| format!("no acid fixture for {:?}", meta.format))?;
        // Import the *supplied* bytes (not the embedded copy), so the consumer
        // also works for on-disk corpus fixtures of the same formats.
        import_bytes(*all, &fixture.bytes())
    }

    fn export(&self, _model: &Imported, format: corpus::Format) -> Result<Vec<u8>, String> {
        Err(format!(
            "the acid corpus is import-fidelity only; export for {format:?} \
             is exercised by the per-crate conformance round-trip suites"
        ))
    }
}

/// Imports `bytes` with the importer for `fixture`'s format.
fn import_bytes(fixture: Fixture, bytes: &[u8]) -> Result<Imported, String> {
    let cursor = Cursor::new(bytes);
    match fixture {
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
        Fixture::Pptx => PptxImport::import(cursor, PptxImportOptions::default())
            .map(|p| Imported::Presentation(Box::new(p)))
            .map_err(|e| e.to_string()),
        Fixture::Odp | Fixture::Odg => Err("no importer yet for this ODF format".to_string()),
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

    /// The shared `Fixture`/`Consumer` traits (Spec 02 B-8) drive the acid
    /// corpus end-to-end: metadata, axis applicability, import dispatch, and
    /// the honestly-unsupported export.
    #[test]
    fn shared_traits_drive_the_acid_corpus() {
        use appthere_conformance::corpus::Consumer as _;

        let consumer = AcidConsumer;
        let docx = Fixture::Docx;
        let meta = corpus::Fixture::meta(&docx);
        assert_eq!(meta.format, corpus::Format::Docx);
        assert!(
            meta.axes.contains(&Axis::RoundTrip),
            "importer ⇒ round-trip"
        );

        let model = consumer.import(&docx).expect("docx imports via the trait");
        assert!(matches!(model, Imported::Document(_)));
        assert!(
            consumer.export(&model, corpus::Format::Docx).is_err(),
            "the acid corpus is import-only; export must say so, not pretend"
        );

        // No ODP importer yet: visual-only axes, documented import failure.
        let odp = Fixture::Odp;
        assert_eq!(corpus::Fixture::meta(&odp).axes, &[Axis::Visual]);
        assert!(consumer.import(&odp).is_err());
    }

    #[test]
    fn fixture_formats_are_distinct() {
        let mut formats: Vec<Format> = Fixture::all().iter().map(|f| f.format()).collect();
        formats.sort_by_key(|f| format!("{f:?}"));
        formats.dedup_by_key(|f| format!("{f:?}"));
        assert_eq!(formats.len(), Fixture::all().len());
    }
}
