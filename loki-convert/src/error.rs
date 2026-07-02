// SPDX-License-Identifier: Apache-2.0

//! Typed conversion errors — the canonical rejection is
//! [`ConvertError::ConversionUnsupported`] (headless spec §3).

use crate::format::Format;

/// Conversion failures.
#[derive(Debug, thiserror::Error)]
pub enum ConvertError {
    /// The pair is not in the capability matrix (ADR-C024): either no code
    /// path exists, or the pair is deliberately gated (PPTX/ODP/ODG,
    /// ratified decision §5.1). Never silently lossy.
    // (Field is `source_format`, not `source` — thiserror reserves the
    // latter name for the error-source chain.)
    #[error("conversion from {source_format} to {target_format} is not supported: {reason}")]
    ConversionUnsupported {
        /// Source format.
        source_format: Format,
        /// Target format.
        target_format: Format,
        /// Why the pair is absent (missing importer/exporter or gated).
        reason: &'static str,
    },
    /// A format name/extension was not recognised.
    #[error("unknown format {0:?}")]
    UnknownFormat(String),
    /// A `--profile` value was not recognised.
    #[error("unknown PDF profile {0:?} (expected pdf, pdf-x1a, pdf-x3, pdf-x4, or pdf-a2b)")]
    UnknownProfile(String),
    /// The profile is recognised but this engine cannot emit it yet.
    #[error("PDF profile {0} is not yet supported by the PDF engine")]
    ProfileUnsupported(&'static str),
    /// A PDF profile was supplied for a non-PDF target.
    #[error("a PDF profile applies only when the target format is pdf")]
    ProfileWithoutPdfTarget,
    /// OOXML import/export failed.
    #[error("ooxml error: {0}")]
    Ooxml(#[from] loki_ooxml::error::OoxmlError),
    /// ODF import/export failed.
    #[error("odf error: {0}")]
    Odf(#[from] loki_odf::error::OdfError),
    /// EPUB export failed.
    #[error("epub error: {0}")]
    Epub(#[from] loki_epub::EpubError),
    /// PDF export failed.
    #[error("pdf error: {0}")]
    Pdf(#[from] loki_pdf::PdfError),
}
