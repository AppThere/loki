// Copyright 2024-2026 AppThere
//
// Licensed under the Apache License, Version 2.0 (the "License");
// you may not use this file except in compliance with the License.
// You may obtain a copy of the License at
//
//     http://www.apache.org/licenses/LICENSE-2.0
//
// Unless required by applicable law or agreed to in writing, software
// distributed under the License is distributed on an "AS IS" BASIS,
// WITHOUT WARRANTIES OR CONDITIONS OF ANY KIND, either express or implied.
// See the License for the specific language governing permissions and
// limitations under the License.

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
//! | `pptx`  | no      | PPTX (PowerPoint) — **not yet implemented** |
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
