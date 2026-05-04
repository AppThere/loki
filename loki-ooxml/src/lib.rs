// Copyright 2026 AppThere Loki contributors
// SPDX-License-Identifier: Apache-2.0

//! OOXML (DOCX/XLSX/PPTX) import/export for the Loki document suite.
//!
//! `loki-ooxml` provides format-specific import and export adapters for the
//! Office Open XML container formats. The primary supported format is DOCX
//! (Word Processing ML, ECMA-376 §17). XLSX and PPTX support is planned for
//! future releases.
//!
//! # Feature flags
//!
//! | Feature | Default | Description |
//! |---------|---------|-------------|
//! | `docx`  | yes     | DOCX (Word) import via [`docx::import::DocxImport`] |
//! | `xlsx`  | no      | XLSX (Excel) — **not yet implemented** |
//! | `pptx`  | no      | PPTX (`PowerPoint`) — **not yet implemented** |
//!
//! # Quick start
//!
//! ```no_run
//! use std::fs::File;
//! use loki_ooxml::docx::import::{DocxImport, DocxImportOptions};
//! use loki_doc_model::io::DocumentImport;
//!
//! let file = File::open("document.docx").unwrap();
//! let doc = DocxImport::import(file, DocxImportOptions::default()).unwrap();
//! ```

#![forbid(unsafe_code)]
#![warn(missing_docs)]
#![warn(clippy::pedantic)]
#![allow(clippy::module_name_repetitions)]
#![allow(clippy::must_use_candidate)]

pub mod constants;
pub mod error;
pub(crate) mod xml_util;

#[cfg(feature = "docx")]
pub mod docx;

#[cfg(feature = "xlsx")]
compile_error!("`xlsx` feature is not yet implemented in loki-ooxml v0.1.0");

#[cfg(feature = "pptx")]
compile_error!("`pptx` feature is not yet implemented in loki-ooxml v0.1.0");

pub use error::{NoteKind, OoxmlError, OoxmlResult, OoxmlWarning};

#[cfg(feature = "docx")]
pub use docx::export::DocxExport;
#[cfg(feature = "docx")]
pub use docx::import::{DocxImport, DocxImportOptions, DocxImportResult};
#[cfg(feature = "docx")]
pub use docx::mapper::{map_document, MapperError};
