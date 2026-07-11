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
/// [`Self::icc_profile`] is embedded as the `DestOutputProfile` (an embedded
/// ICC profile in the output intent is required for strict PDF/X-3 and PDF/X-4).
/// The default embeds a bundled, **public-domain (CC0)** compact CMYK profile
/// characterising CGATS TR 001 (the U.S. SWOP coated-web reference — see
/// `assets/README.md`), so every export carries an embedded CMYK
/// characterisation out of the box. For certified press output against a
/// specific condition (e.g. a licensed ISO Coated / FOGRA profile), override it
/// with [`Self::with_icc_profile`]. Setting it to `None` falls back to
/// referencing the named condition only (an X-1a-style intent).
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

/// The bundled default CMYK output profile: a compact, CC0-licensed profile
/// characterising CGATS TR 001-1995 (U.S. SWOP). See `assets/README.md`.
const DEFAULT_CMYK_ICC: &[u8] = include_bytes!("../assets/CGATS001Compat-v2-micro.icc");

impl Default for OutputIntent {
    fn default() -> Self {
        Self {
            condition_identifier: "CGATS TR 001".to_string(),
            condition: Some("CGATS TR 001-1995 (SWOP), coated web".to_string()),
            registry_name: Some("http://www.color.org".to_string()),
            info: Some("CGATS TR 001 (bundled CC0 compact CMYK profile)".to_string()),
            icc_profile: Some(DEFAULT_CMYK_ICC.to_vec()),
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
    fn default_intent_embeds_bundled_cmyk_profile() {
        let oi = OutputIntent::default();
        assert_eq!(oi.condition_identifier, "CGATS TR 001");
        // The bundled CC0 profile is embedded by default and is a valid CMYK
        // ICC profile (magic 'acsp' at offset 36, data color space 'CMYK').
        let icc = oi.icc_profile.expect("a default CMYK profile is bundled");
        assert_eq!(&icc[36..40], b"acsp", "not a valid ICC profile");
        assert_eq!(&icc[16..20], b"CMYK", "output intent profile must be CMYK");
    }

    #[test]
    fn with_icc_profile_sets_the_dest_output_profile() {
        let oi = OutputIntent::default().with_icc_profile(vec![1, 2, 3, 4]);
        assert_eq!(oi.icc_profile.as_deref(), Some([1, 2, 3, 4].as_slice()));
    }
}
