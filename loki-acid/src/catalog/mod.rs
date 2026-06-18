// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 AppThere Loki contributors

//! The ACID test-case catalog — the machine-readable transcription of
//! `TEST_PLAN.md`.
//!
//! Each [`TestCase`] is a construct that office-suite alternatives are known to
//! render differently from the canonical Microsoft 365 (OOXML) or LibreOffice
//! (ODF) render. The catalog is the single source of truth the harness reports
//! against; page indices are intentionally omitted until golden references pin
//! each case to a page.

mod docx;
mod odf;
mod pptx;
mod xlsx;

use serde::{Deserialize, Serialize};

use crate::severity::Severity;

/// The document format a test case belongs to.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Format {
    /// Word-processing OOXML (`.docx`).
    Docx,
    /// Spreadsheet OOXML (`.xlsx`).
    Xlsx,
    /// Presentation OOXML (`.pptx`).
    Pptx,
    /// OpenDocument Text (`.odt`).
    Odt,
    /// OpenDocument Presentation (`.odp`).
    Odp,
    /// OpenDocument Graphics (`.odg`).
    Odg,
    /// OpenDocument Spreadsheet (`.ods`).
    Ods,
}

impl Format {
    /// The canonical reference render authority for this format.
    ///
    /// OOXML formats diff against Microsoft 365; ODF formats against
    /// LibreOffice (see the ODF note in the plan).
    #[must_use]
    pub fn canonical_authority(self) -> &'static str {
        match self {
            Format::Docx | Format::Xlsx | Format::Pptx => "Microsoft 365",
            Format::Odt | Format::Odp | Format::Odg | Format::Ods => "LibreOffice",
        }
    }
}

/// A single acid-test construct.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct TestCase {
    /// Stable identifier, e.g. `TC-DOCX-014`.
    pub id: &'static str,
    /// Owning format.
    pub format: Format,
    /// Severity if Loki diverges from canonical.
    pub severity: Severity,
    /// Short description of the construct under test.
    pub feature: &'static str,
}

/// Terse constructor used by the per-format data tables.
const fn tc(
    id: &'static str,
    format: Format,
    severity: Severity,
    feature: &'static str,
) -> TestCase {
    TestCase {
        id,
        format,
        severity,
        feature,
    }
}

/// Returns every catalogued test case across all formats.
#[must_use]
pub fn all_cases() -> Vec<TestCase> {
    let mut cases = Vec::new();
    cases.extend_from_slice(docx::CASES);
    cases.extend_from_slice(xlsx::CASES);
    cases.extend_from_slice(pptx::CASES);
    cases.extend_from_slice(odf::CASES);
    cases
}

/// Returns the catalogued cases for a single format.
#[must_use]
pub fn cases_for(format: Format) -> Vec<TestCase> {
    all_cases()
        .into_iter()
        .filter(|c| c.format == format)
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ids_are_unique() {
        let cases = all_cases();
        let mut ids: Vec<&str> = cases.iter().map(|c| c.id).collect();
        ids.sort_unstable();
        let before = ids.len();
        ids.dedup();
        assert_eq!(before, ids.len(), "duplicate test-case id in catalog");
    }

    #[test]
    fn catalog_covers_every_format() {
        for format in [
            Format::Docx,
            Format::Xlsx,
            Format::Pptx,
            Format::Odt,
            Format::Odp,
            Format::Odg,
            Format::Ods,
        ] {
            assert!(
                !cases_for(format).is_empty(),
                "no cases catalogued for {format:?}"
            );
        }
    }

    #[test]
    fn counts_match_plan_totals() {
        // Totals transcribed from TEST_PLAN.md section headers.
        assert_eq!(cases_for(Format::Docx).len(), 38);
        assert_eq!(cases_for(Format::Xlsx).len(), 30);
        assert_eq!(cases_for(Format::Pptx).len(), 29);
        assert_eq!(cases_for(Format::Odt).len(), 14);
        assert_eq!(cases_for(Format::Odp).len(), 9);
        assert_eq!(cases_for(Format::Odg).len(), 9);
        assert_eq!(cases_for(Format::Ods).len(), 10);
    }
}
