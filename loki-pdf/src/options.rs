// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 AppThere Loki contributors

//! PDF-X conformance options.
//!
//! The exporter always renders text and graphics in **DeviceCMYK** and attaches
//! a CMYK [`OutputIntent`], because every supported PDF/X level is valid with a
//! CMYK-only colour workflow and that is what print publication requires.  The
//! [`PdfXLevel`] then selects the PDF version, the `GTS_PDFXVersion` marker, and
//! the XMP conformance claim.

/// The targeted PDF/X conformance level.
///
/// All three print-oriented levels are supported. They differ in PDF base
/// version and the allowed feature set; the exporter restricts itself to the
/// common subset (CMYK colour, embedded fonts, no transparency) so a single
/// content pipeline satisfies every level.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
#[non_exhaustive]
pub enum PdfXLevel {
    /// PDF/X-1a:2003 — CMYK / spot only, PDF 1.4, no transparency, no RGB.
    #[default]
    X1a,
    /// PDF/X-3:2003 — CMYK plus ICC-tagged colour, PDF 1.4, no transparency.
    X3,
    /// PDF/X-4 — PDF 1.6, live transparency and layers permitted.
    X4,
}

impl PdfXLevel {
    /// The `GTS_PDFXVersion` / XMP conformance string for this level.
    #[must_use]
    pub fn version_string(self) -> &'static str {
        match self {
            PdfXLevel::X1a => "PDF/X-1a:2003",
            PdfXLevel::X3 => "PDF/X-3:2003",
            PdfXLevel::X4 => "PDF/X-4",
        }
    }

    /// The `pdfxid:GTS_PDFXConformance` value, where applicable.
    #[must_use]
    pub fn conformance_string(self) -> &'static str {
        match self {
            PdfXLevel::X1a => "PDF/X-1a:2003",
            PdfXLevel::X3 => "PDF/X-3:2003",
            PdfXLevel::X4 => "PDF/X-4",
        }
    }

    /// The PDF base version (major, minor) required by this level.
    #[must_use]
    pub fn pdf_version(self) -> (u8, u8) {
        match self {
            PdfXLevel::X1a | PdfXLevel::X3 => (1, 4),
            PdfXLevel::X4 => (1, 6),
        }
    }
}

/// Describes the printing condition referenced by the PDF/X `OutputIntent`.
///
/// When [`Self::icc_profile`] is provided it is embedded as the
/// `DestOutputProfile`. **This is required for strict PDF/X-3 and PDF/X-4
/// conformance** (both mandate an embedded ICC profile in the output intent).
/// When it is `None`, the output intent references the named printing condition
/// only — accepted for PDF/X-1a when [`Self::condition_identifier`] is a
/// registered characterisation from the ICC registry, but *not* a conformant
/// X-3/X-4 file. The default targets the widely used FOGRA39 coated-paper
/// condition; supply a matching profile with [`Self::with_icc_profile`] for
/// certified output.
///
/// TODO(pdf-icc-default-profile): no CMYK ICC profile is bundled by default
/// (embedding one is a licensing/asset decision — profiles carry their own
/// redistribution terms). Until a profile is bundled, X-3/X-4 callers must
/// supply one via [`Self::with_icc_profile`]; a build that bundles an approved
/// profile (e.g. an ECI/FOGRA release) can default `icc_profile` to it.
#[derive(Debug, Clone)]
pub struct OutputIntent {
    /// `OutputConditionIdentifier` — a registered condition name (e.g.
    /// `"FOGRA39"`) or a free-form identifier when a profile is embedded.
    pub condition_identifier: String,
    /// Human-readable `OutputCondition` description.
    pub condition: Option<String>,
    /// `RegistryName` — the URL of the registry hosting the condition.
    pub registry_name: Option<String>,
    /// Additional `Info` string.
    pub info: Option<String>,
    /// Optional embedded ICC profile bytes (a 4-component CMYK profile). When
    /// present it is written as the `DestOutputProfile` stream — required for
    /// strict PDF/X-3 / X-4 conformance.
    pub icc_profile: Option<Vec<u8>>,
}

impl OutputIntent {
    /// Sets the embedded CMYK ICC profile (the `DestOutputProfile`). Use this to
    /// make the output intent conformant for PDF/X-3 / X-4. The bytes must be a
    /// 4-component (CMYK) ICC profile matching `condition_identifier`.
    #[must_use]
    pub fn with_icc_profile(mut self, profile: Vec<u8>) -> Self {
        self.icc_profile = Some(profile);
        self
    }
}

impl Default for OutputIntent {
    fn default() -> Self {
        Self {
            condition_identifier: "FOGRA39".to_string(),
            condition: Some("Coated FOGRA39 (ISO 12647-2:2004)".to_string()),
            registry_name: Some("http://www.color.org".to_string()),
            info: Some("Coated FOGRA39 (ISO 12647-2:2004)".to_string()),
            // See TODO(pdf-icc-default-profile) above — integrator-supplied.
            icc_profile: None,
        }
    }
}

/// Options for a PDF/X export.
#[derive(Debug, Clone, Default)]
pub struct PdfXOptions {
    /// Targeted conformance level.
    pub level: PdfXLevel,
    /// Overrides the document title in the PDF metadata (falls back to the
    /// document's own title when `None`).
    pub title: Option<String>,
    /// The output intent / printing condition.
    pub output_intent: OutputIntent,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn version_strings() {
        assert_eq!(PdfXLevel::X1a.version_string(), "PDF/X-1a:2003");
        assert_eq!(PdfXLevel::X4.pdf_version(), (1, 6));
        assert_eq!(PdfXLevel::X3.pdf_version(), (1, 4));
    }

    #[test]
    fn default_intent_is_fogra39() {
        let oi = OutputIntent::default();
        assert_eq!(oi.condition_identifier, "FOGRA39");
        assert!(oi.icc_profile.is_none());
    }

    #[test]
    fn with_icc_profile_sets_the_dest_output_profile() {
        let oi = OutputIntent::default().with_icc_profile(vec![1, 2, 3, 4]);
        assert_eq!(oi.icc_profile.as_deref(), Some([1, 2, 3, 4].as_slice()));
    }
}
