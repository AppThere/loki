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

//! ODT export — stub implementation.
//!
//! [`OdtExport`] will write a [`loki_doc_model::Document`] to an ODT
//! (OpenDocument Text) ZIP package. The full implementation is deferred to a
//! later session; calling [`OdtExport::export`] currently returns
//! [`crate::error::OdfError::NotImplemented`].

use std::io::{Seek, Write};

use loki_doc_model::document::Document;
use loki_doc_model::io::DocumentExport;

use crate::error::{OdfError, OdfResult};

/// Options controlling ODT export behaviour.
///
/// Currently empty; reserved for future use (e.g. controlling whether
/// images are embedded or linked).
///
/// ODF 1.3 §3 (package conventions).
#[non_exhaustive]
#[derive(Debug, Clone, Default)]
pub struct OdtExportOptions {}

/// Unit struct that implements [`DocumentExport`] for ODT files.
///
/// Export is not yet implemented; all calls return
/// [`OdfError::NotImplemented`]. ODF 1.3 §3.
pub struct OdtExport;

impl DocumentExport for OdtExport {
    type Error = OdfError;
    type Options = OdtExportOptions;

    /// Export a [`Document`] as an ODT file.
    ///
    /// **Not yet implemented.** Returns [`OdfError::NotImplemented`].
    ///
    /// ODF 1.3 §3 (package conventions).
    fn export(
        _doc: &Document,
        _writer: impl Write + Seek,
        _options: Self::Options,
    ) -> OdfResult<()> {
        Err(OdfError::NotImplemented { feature: "ODT export".into() })
    }
}
