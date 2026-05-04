// Copyright 2026 AppThere Loki contributors
// SPDX-License-Identifier: Apache-2.0

//! ODF (`OpenDocument`) import/export for the Loki document suite.
//!
//! `loki-odf` provides format-specific import and export adapters for the
//! `OpenDocument` Text (ODT) format. It produces and consumes
//! [`loki_doc_model`] types and enforces the **version-preserving
//! round-trip rule**: a document loaded from an ODF 1.1 file is exported
//! as ODF 1.1, and so on.
//!
//! # Supported formats
//!
//! | Format | Status |
//! |--------|--------|
//! | ODT (text) import | skeleton — content parsing in later sessions |
//! | ODT (text) export | **not yet implemented** |
//!
//! # Quick start
//!
//! ```no_run
//! use std::fs::File;
//! use loki_odf::odt::import::{OdtImport, OdtImportOptions};
//! use loki_doc_model::io::DocumentImport;
//!
//! let file = File::open("document.odt").unwrap();
//! let doc = OdtImport::import(file, OdtImportOptions::default()).unwrap();
//! ```
//!
//! # Version round-trip
//!
//! ODF 1.3 §3 — the `office:version` attribute on the root element is the
//! authoritative version marker. Import records the detected version in
//! [`loki_doc_model::io::DocumentSource::version`]; export reads it and
//! emits accordingly.

#![forbid(unsafe_code)]
#![warn(missing_docs)]
#![warn(clippy::pedantic)]
#![allow(clippy::module_name_repetitions)]
#![allow(clippy::must_use_candidate)]

pub mod constants;
pub mod error;
pub mod odt;
pub mod package;
pub mod version;
pub(crate) mod xml_util;

pub use error::{OdfError, OdfResult, OdfWarning};
pub use odt::export::OdtExport;
pub use odt::import::{OdtImport, OdtImportOptions, OdtImportResult};
pub use version::OdfVersion;
