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

//! ODT (OpenDocument Text) import and export.
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
//! trait). Export is not yet implemented; all calls return
//! [`crate::error::OdfError::NotImplemented`].
//!
//! # Version round-trip
//!
//! A document loaded from an ODF 1.1 file is exported as ODF 1.1;
//! an ODF 1.2 file is exported as ODF 1.2; etc. The version is carried via
//! [`loki_doc_model::io::DocumentSource::version`].

pub mod export;
pub mod import;
