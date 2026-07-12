// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 AppThere Loki contributors

//! The shared fixture corpus vocabulary (Spec 02 §9 / B-8, B-9).
//!
//! Promoted from `loki-acid` so every suite app (Text today; Presentation and
//! Spreadsheet later) shares one severity scale, format enum, test-case
//! catalog, and fixture metadata schema — with **no Text-specific
//! assumptions**. A consumer supplies documents through the [`Fixture`] trait
//! and its import/export pair through the [`Consumer`] trait, and drives the
//! three axes with them.
//!
//! The corpus is organised **feature × format × axis**: every catalogued
//! [`TestCase`] names its feature and format (the catalog is the
//! machine-readable `TEST_PLAN.md`, 141 `TC-*` constructs), and every
//! [`FixtureMeta`] records which [`Axis`]es apply, the reference application
//! and version behind its golden, and any per-fixture tolerance override with
//! its justification.

pub mod catalog;
pub mod manifest;

use std::borrow::Cow;

use serde::{Deserialize, Serialize};

/// Severity of a fidelity divergence, per the master test plan.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
pub enum Severity {
    /// Silent data/layout corruption a reader notices immediately (wrong merge,
    /// dropped text, wrong page count, garbled glyphs).
    P0,
    /// Visible fidelity gap (wrong spacing, colour, wrap) a careful reader
    /// catches.
    P1,
    /// Subtle metric / typographic drift.
    P2,
}

impl Severity {
    /// Short uppercase label (`"P0"`, `"P1"`, `"P2"`).
    #[must_use]
    pub fn label(self) -> &'static str {
        match self {
            Severity::P0 => "P0",
            Severity::P1 => "P1",
            Severity::P2 => "P2",
        }
    }
}

/// The document format a fixture or test case belongs to.
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
    /// LibreOffice.
    #[must_use]
    pub fn canonical_authority(self) -> &'static str {
        match self {
            Format::Docx | Format::Xlsx | Format::Pptx => "Microsoft 365",
            Format::Odt | Format::Odp | Format::Odg | Format::Ods => "LibreOffice",
        }
    }
}

/// One of the three independent conformance axes (Spec 02 §4).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum Axis {
    /// Exported XML validates against the official OOXML/ODF schemas.
    Schema,
    /// Import → export → re-import preserves the normalized model.
    RoundTrip,
    /// The render matches a committed golden within calibrated tolerance.
    Visual,
}

/// The reference application (and pinned version) behind a fixture's golden.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
pub struct Reference {
    /// Application name, e.g. `"LibreOffice"` or `"Microsoft 365"`.
    pub app: &'static str,
    /// Version the golden was generated with, e.g. `"24.2"`.
    pub version: &'static str,
}

/// A per-fixture visual-tolerance override, always with its justification
/// (Spec 02 §9: overrides are data, not folklore).
#[derive(Debug, Clone, Copy, PartialEq, Serialize)]
pub struct ToleranceOverride {
    /// Overriding minimum regional SSIM (default: `Tolerance::calibrated()`).
    pub min_ssim: f64,
    /// Overriding maximum regional mean ΔE (CIEDE2000).
    pub max_delta_e: f64,
    /// Why this fixture deviates from the calibrated corpus-wide tolerance.
    pub justification: &'static str,
}

/// A single acid-test construct (a `TC-*` entry of the master plan).
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

/// Terse constructor used by the per-format catalog tables.
pub(crate) const fn tc(
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

/// The recorded metadata of one corpus fixture (Spec 02 §9): the feature it
/// exercises, which axes apply, the reference behind its golden, and any
/// justified tolerance override. Compile-time data (like the catalog), so it
/// serializes into reports but is never deserialized.
#[derive(Debug, Clone, Copy, PartialEq, Serialize)]
pub struct FixtureMeta {
    /// Stable fixture id — also its on-disk stem, e.g. `para-carlito`.
    pub id: &'static str,
    /// Document format.
    pub format: Format,
    /// The feature (or feature set) the fixture exercises.
    pub feature: &'static str,
    /// Severity of a divergence on this fixture.
    pub severity: Severity,
    /// Which conformance axes apply to this fixture.
    pub axes: &'static [Axis],
    /// Reference application/version behind the golden; `None` when the
    /// fixture has no visual golden (non-[`Axis::Visual`] fixtures).
    pub reference: Option<Reference>,
    /// Per-fixture tolerance override; `None` = the calibrated default.
    pub tolerance_override: Option<ToleranceOverride>,
}

/// A conformance fixture: metadata plus the document bytes.
///
/// Implemented by on-disk corpus entries ([`manifest::DiskFixture`]) and by
/// consumers with embedded fixtures (e.g. `loki-acid`'s acid documents).
pub trait Fixture {
    /// The fixture's recorded metadata.
    fn meta(&self) -> FixtureMeta;
    /// The document bytes (borrowed for embedded fixtures, owned for disk).
    fn bytes(&self) -> Cow<'_, [u8]>;
}

/// A suite app consuming the harness: supplies the import/export pair the
/// axes drive (Spec 02 §8). The model type is the consumer's own; round-trip
/// comparison goes through its `NormalizedModel` impl.
pub trait Consumer {
    /// The consumer's imported model (e.g. a document, workbook, or
    /// presentation — or an enum over them).
    type Model;

    /// Imports `fixture` into the model, or a human-readable failure
    /// (including the documented "no importer yet" case).
    fn import(&self, fixture: &dyn Fixture) -> Result<Self::Model, String>;

    /// Exports `model` back to `format` for the schema and round-trip axes.
    /// Import-only corpora return `Err` describing the limitation.
    fn export(&self, model: &Self::Model, format: Format) -> Result<Vec<u8>, String>;
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn authority_split_matches_the_plan() {
        assert_eq!(Format::Docx.canonical_authority(), "Microsoft 365");
        assert_eq!(Format::Odt.canonical_authority(), "LibreOffice");
    }

    #[test]
    fn severity_labels() {
        assert_eq!(Severity::P0.label(), "P0");
        assert_eq!(Severity::P2.label(), "P2");
    }
}
