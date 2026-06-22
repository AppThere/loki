// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 AppThere Loki contributors

//! ODT (`OpenDocument` Text) import and export.
//!
//! # Import
//!
//! Use [`import::OdtImport`] (via the [`loki_doc_model::io::DocumentImport`]
//! trait) for simple use cases, or [`import::OdtImporter`] when you need
//! access to non-fatal [`crate::error::OdfWarning`]s and the detected
//! [`crate::version::OdfVersion`].
//!
//! # Export
//!
//! Use [`export::OdtExport`] (via the [`loki_doc_model::io::DocumentExport`]
//! trait). It writes the `content.xml`, `styles.xml`, and `meta.xml` parts of
//! an ODT package — paragraphs, headings, styled paragraphs, lists, tables,
//! inline formatting, the named style catalog, page geometry, and metadata.
//!
//! # Version round-trip
//!
//! A document loaded from an ODF 1.1 file is exported as ODF 1.1;
//! an ODF 1.2 file is exported as ODF 1.2; etc. The version is carried via
//! [`loki_doc_model::io::DocumentSource::version`].

pub mod export;
pub mod import;
pub(crate) mod mapper;
pub(crate) mod model;
pub(crate) mod reader;
pub(crate) mod write;
