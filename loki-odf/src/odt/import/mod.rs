// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 AppThere Loki contributors

//! ODT import entry point.
//!
//! [`OdtImport`] implements [`loki_doc_model::io::DocumentImport`] and is the
//! primary public API for converting an ODT file into a
//! [`loki_doc_model::Document`].
//!
//! The current implementation opens and validates the ODF package and records
//! the source version; document content parsing will be added in later
//! sessions.
//!
//! # Round-trip version rule
//!
//! The detected [`OdfVersion`] is stored in
//! [`OdtImportResult::source_version`] and written into the document's
//! [`loki_doc_model::io::DocumentSource`]. Exporters read this field so that
//! a document round-tripped through this crate is emitted at the same ODF
//! version as its source.

mod importer;
mod options;
mod types;
mod version_attr;

#[cfg(test)]
mod tests;

pub use importer::{OdtImport, OdtImporter};
pub use options::OdtImportOptions;
pub use types::OdtImportResult;
