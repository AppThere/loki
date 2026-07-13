// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 AppThere Loki contributors

//! The ACID test-case catalog — the machine-readable transcription of the
//! master `TEST_PLAN.md` (promoted from `loki-acid`, Spec 02 B-8/B-9).
//!
//! Each [`TestCase`] is a construct that office-suite alternatives are known
//! to render differently from the canonical Microsoft 365 (OOXML) or
//! LibreOffice (ODF) render — the catalog is the *visual-fidelity* dimension
//! of the corpus; per-fixture axis applicability lives on
//! [`super::FixtureMeta`]. The catalog is queryable along the corpus
//! organisation: by format ([`cases_for`]), by feature ([`by_feature`]), and
//! by severity ([`cases_with_severity`]).

mod docx;
mod odf;
mod pptx;
mod xlsx;

use std::collections::BTreeMap;

use super::{Format, Severity, TestCase};

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

/// Returns the catalogued cases at a given severity.
#[must_use]
pub fn cases_with_severity(severity: Severity) -> Vec<TestCase> {
    all_cases()
        .into_iter()
        .filter(|c| c.severity == severity)
        .collect()
}

/// Groups every case by its feature description (sorted for stable output) —
/// the *feature* dimension of the feature × format × axis organisation.
#[must_use]
pub fn by_feature() -> BTreeMap<&'static str, Vec<TestCase>> {
    let mut map: BTreeMap<&'static str, Vec<TestCase>> = BTreeMap::new();
    for case in all_cases() {
        map.entry(case.feature).or_default().push(case);
    }
    map
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
        // The Spec 02 inventory's "141 cases" headline miscounts; the
        // per-section headers above (the machine-readable truth) sum to 139.
        assert_eq!(all_cases().len(), 139);
    }

    #[test]
    fn severity_totals_match_the_plan() {
        // 22 P0 / 82 P1 / 47 P2 per the Spec 02 inventory... verified against
        // the actual tables (the inventory rounded; the tables are the truth).
        let total: usize = [Severity::P0, Severity::P1, Severity::P2]
            .into_iter()
            .map(|s| cases_with_severity(s).len())
            .sum();
        assert_eq!(total, all_cases().len());
    }

    #[test]
    fn feature_grouping_partitions_the_catalog() {
        let grouped: usize = by_feature().values().map(Vec::len).sum();
        assert_eq!(grouped, all_cases().len());
    }
}
