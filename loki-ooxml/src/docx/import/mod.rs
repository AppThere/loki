// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 AppThere Loki contributors

//! DOCX import entry point.
//!
//! [`DocxImport`] implements [`loki_doc_model::io::DocumentImport`] and is
//! the primary public API for converting a DOCX file into a
//! [`loki_doc_model::Document`].
//!
//! # Example
//!
//! ```no_run
//! use std::fs::File;
//! use loki_ooxml::docx::import::{DocxImport, DocxImportOptions};
//! use loki_doc_model::io::DocumentImport;
//!
//! let file = File::open("document.docx").unwrap();
//! let doc = DocxImport::import(file, DocxImportOptions::default()).unwrap();
//! ```

mod helpers;
mod importer;
mod options;
mod pipeline;

pub use importer::{DocxImport, DocxImporter};
pub use options::{DocxImportOptions, DocxImportResult};
pub(crate) use pipeline::parse_and_map_package;
