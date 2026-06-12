// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 AppThere Loki contributors

//! Import options for ODT files.

// ── Options ────────────────────────────────────────────────────────────────────

/// Options controlling ODT import behaviour.
///
/// ODF 1.3 §3 (package conventions).
#[non_exhaustive]
#[derive(Debug, Clone)]
pub struct OdtImportOptions {
    /// When `true`, paragraphs whose style name starts with `"Heading"` are
    /// mapped to [`loki_doc_model::content::block::Block::Heading`] rather
    /// than plain paragraph blocks.
    ///
    /// Defaults to `true`.
    pub emit_heading_blocks: bool,

    /// When `true`, images are embedded in the document as data URIs
    /// (`data:<media-type>;base64,<data>`). When `false`, images are omitted.
    ///
    /// Defaults to `true`.
    pub embed_images: bool,

    /// When `true`, an unrecognised `office:version` attribute causes
    /// [`crate::error::OdfError::UnsupportedVersion`] to be returned. When `false`
    /// (default), the version is treated as the latest supported version and
    /// an [`crate::error::OdfWarning::UnrecognisedVersion`] is emitted instead.
    ///
    /// ODF 1.3 §3 (`office:version` attribute).
    pub strict_version: bool,
}

impl Default for OdtImportOptions {
    fn default() -> Self {
        Self {
            emit_heading_blocks: true,
            embed_images: true,
            strict_version: false,
        }
    }
}
