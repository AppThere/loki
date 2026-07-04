// SPDX-License-Identifier: Apache-2.0

//! File formats and PDF conformance profiles.

/// A file format Loki can (potentially) convert between.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Format {
    /// Word document (OOXML).
    Docx,
    /// OpenDocument text.
    Odt,
    /// Excel workbook (OOXML).
    Xlsx,
    /// OpenDocument spreadsheet.
    Ods,
    /// PowerPoint presentation (OOXML) — gated, see ratified decision §5.1.
    Pptx,
    /// OpenDocument presentation — gated alongside PPTX.
    Odp,
    /// OpenDocument graphics — gated alongside PPTX.
    Odg,
    /// EPUB 3.3 (export only).
    Epub,
    /// PDF (export only; profile selected via [`crate::PdfProfile`]).
    Pdf,
}

impl Format {
    /// All formats, for capability-table iteration.
    pub const ALL: [Format; 9] = [
        Format::Docx,
        Format::Odt,
        Format::Xlsx,
        Format::Ods,
        Format::Pptx,
        Format::Odp,
        Format::Odg,
        Format::Epub,
        Format::Pdf,
    ];

    /// Guesses the format from a file extension (case-insensitive).
    #[must_use]
    pub fn from_extension(ext: &str) -> Option<Format> {
        match ext.to_ascii_lowercase().as_str() {
            "docx" => Some(Format::Docx),
            "odt" => Some(Format::Odt),
            "xlsx" => Some(Format::Xlsx),
            "ods" => Some(Format::Ods),
            "pptx" => Some(Format::Pptx),
            "odp" => Some(Format::Odp),
            "odg" => Some(Format::Odg),
            "epub" => Some(Format::Epub),
            "pdf" => Some(Format::Pdf),
            _ => None,
        }
    }

    /// Canonical lowercase name (matches the file extension).
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Format::Docx => "docx",
            Format::Odt => "odt",
            Format::Xlsx => "xlsx",
            Format::Ods => "ods",
            Format::Pptx => "pptx",
            Format::Odp => "odp",
            Format::Odg => "odg",
            Format::Epub => "epub",
            Format::Pdf => "pdf",
        }
    }
}

impl std::fmt::Display for Format {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.as_str())
    }
}

impl std::str::FromStr for Format {
    type Err = crate::ConvertError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        Format::from_extension(s).ok_or_else(|| crate::ConvertError::UnknownFormat(s.to_owned()))
    }
}

/// PDF conformance profile (headless spec ADR-C022 / §3 `--profile`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum PdfProfile {
    /// General output. Emitted by the current `loki-pdf` engine as its
    /// default (PDF/X-1a-shaped output that any reader accepts).
    #[default]
    Default,
    /// PDF/X-1a — CMYK/spot print production.
    PdfX1a,
    /// PDF/X-3 — CMYK plus ICC-tagged colour.
    PdfX3,
    /// PDF/X-4 — print production with live transparency.
    PdfX4,
    /// PDF/A-2b — archival. Recognised but not yet emitted; see
    /// [`crate::ConvertError::ProfileUnsupported`].
    PdfA2b,
}

impl PdfProfile {
    /// CLI/API token (`pdf-x4`, `pdf-a2b`, …).
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            PdfProfile::Default => "pdf",
            PdfProfile::PdfX1a => "pdf-x1a",
            PdfProfile::PdfX3 => "pdf-x3",
            PdfProfile::PdfX4 => "pdf-x4",
            PdfProfile::PdfA2b => "pdf-a2b",
        }
    }
}

impl std::str::FromStr for PdfProfile {
    type Err = crate::ConvertError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "pdf" => Ok(PdfProfile::Default),
            "pdf-x1a" => Ok(PdfProfile::PdfX1a),
            "pdf-x3" => Ok(PdfProfile::PdfX3),
            "pdf-x4" => Ok(PdfProfile::PdfX4),
            "pdf-a2b" => Ok(PdfProfile::PdfA2b),
            other => Err(crate::ConvertError::UnknownProfile(other.to_owned())),
        }
    }
}
