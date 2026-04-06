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

//! Import and export traits for format-specific crates.
//!
//! `loki-odf` and `loki-ooxml` implement [`DocumentImport`] and
//! [`DocumentExport`] to convert between their native formats and the
//! abstract [`crate::Document`] model.

pub mod source;

use crate::document::Document;

/// Implemented by format-specific importers (`loki-odf`, `loki-ooxml`).
///
/// A type implementing this trait can parse a byte stream and produce a
/// [`Document`]. Format-specific errors are represented by the associated
/// `Error` type.
pub trait DocumentImport: Sized {
    /// The error type returned by [`Self::import`].
    type Error: std::error::Error + Send + Sync + 'static;
    /// Format-specific import options (use `()` for formats with no options).
    type Options: Default;

    /// Parses a document from the given reader.
    ///
    /// The reader must support both [`std::io::Read`] and
    /// [`std::io::Seek`] because most container formats (ZIP for ODF/OOXML)
    /// require random access.
    ///
    /// # Errors
    ///
    /// Returns an error if the input is malformed or if an unsupported
    /// feature is encountered per the options.
    fn import(
        reader: impl std::io::Read + std::io::Seek,
        options: Self::Options,
    ) -> Result<Document, Self::Error>;
}

/// Implemented by format-specific exporters (`loki-odf`, `loki-ooxml`).
///
/// A type implementing this trait can serialize a [`Document`] into a
/// byte stream. Format-specific errors are represented by the associated
/// `Error` type.
pub trait DocumentExport {
    /// The error type returned by [`Self::export`].
    type Error: std::error::Error + Send + Sync + 'static;
    /// Format-specific export options (use `()` for formats with no options).
    type Options: Default;

    /// Serializes a document to the given writer.
    ///
    /// The writer must support both [`std::io::Write`] and
    /// [`std::io::Seek`] because most container formats (ZIP for ODF/OOXML)
    /// require random access during writing.
    ///
    /// # Errors
    ///
    /// Returns an error if the document cannot be serialized (e.g. if it
    /// references an unsupported feature for the target format).
    fn export(
        doc: &Document,
        writer: impl std::io::Write + std::io::Seek,
        options: Self::Options,
    ) -> Result<(), Self::Error>;
}
