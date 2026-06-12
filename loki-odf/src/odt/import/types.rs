// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 AppThere Loki contributors

//! Result types for ODT import.

use loki_doc_model::document::Document;

use crate::error::OdfWarning;
use crate::version::OdfVersion;

// ── Result ─────────────────────────────────────────────────────────────────────

/// The result of a successful ODT import.
///
/// ODF 1.3 §3 (package conventions).
#[derive(Debug)]
pub struct OdtImportResult {
    /// The imported document in the format-neutral abstract model.
    pub document: Document,

    /// Non-fatal issues encountered during import.
    pub warnings: Vec<OdfWarning>,

    /// The ODF version detected in the source file.
    ///
    /// Exporters should use this value to write the document back at the same
    /// version, preserving the round-trip contract.
    pub source_version: OdfVersion,
}
