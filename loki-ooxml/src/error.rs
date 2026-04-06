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

//! Error and warning types for the `loki-ooxml` crate.
//!
//! [`OoxmlError`] represents fatal import/export failures. [`OoxmlWarning`]
//! collects non-fatal issues that are reported alongside a successfully
//! imported [`loki_doc_model::Document`].

use thiserror::Error;

/// Fatal errors produced during OOXML import or export.
#[non_exhaustive]
#[derive(Debug, Error)]
pub enum OoxmlError {
    /// An error from the OPC packaging layer (`loki-opc`).
    #[error("OPC error: {0}")]
    Opc(#[from] loki_opc::OpcError),

    /// A `quick-xml` parse error in the named part.
    #[error("XML parse error in {part:?}: {source}")]
    Xml {
        /// The OPC part path where the error occurred (e.g. `"word/document.xml"`).
        part: String,
        /// The underlying XML parse error.
        #[source]
        source: quick_xml::Error,
    },

    /// A required OPC part could not be found via its relationship type.
    #[error("missing required part: {relationship_type}")]
    MissingPart {
        /// The OPC relationship type that was searched for.
        relationship_type: String,
    },

    /// A relationship id referenced in the document body was not found.
    #[error("unresolved relationship id {id:?} in {part:?}")]
    UnresolvedRelationship {
        /// The relationship id (e.g. `"rId5"`).
        id: String,
        /// The OPC part that contained the reference.
        part: String,
    },

    /// An XML element was structurally invalid.
    #[error("malformed {element} in {part:?}: {reason}")]
    MalformedElement {
        /// The element name (e.g. `"w:p"`, `"w:sectPr"`).
        element: String,
        /// The OPC part that contained the element.
        part: String,
        /// A human-readable description of the problem.
        reason: String,
    },

    /// DOCX export is not implemented in loki-ooxml v0.1.0.
    #[error("DOCX export is not implemented in loki-ooxml v0.1.0")]
    ExportNotImplemented,

    /// An integer parse error (for attribute values).
    #[error("integer parse error: {0}")]
    ParseInt(#[from] std::num::ParseIntError),

    /// An attribute value that was expected to be a valid UTF-8 string was not.
    #[error("UTF-8 decode error: {0}")]
    Utf8(#[from] std::str::Utf8Error),
}

/// Convenience result alias for loki-ooxml operations.
pub type OoxmlResult<T> = Result<T, OoxmlError>;

/// The kind of note for [`OoxmlWarning::MissingNoteContent`].
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NoteKind {
    /// A footnote appearing at the bottom of the page.
    Footnote,
    /// An endnote appearing at the end of the document.
    Endnote,
}

/// Non-fatal issues encountered during OOXML import.
///
/// Collected alongside the imported [`loki_doc_model::Document`] so that
/// callers can inspect document quality without a fatal error.
#[non_exhaustive]
#[derive(Debug, Clone)]
pub enum OoxmlWarning {
    /// A relationship id referenced in the document body was not found.
    UnresolvedRelationship {
        /// The missing relationship id.
        id: String,
        /// Context describing where the reference appeared.
        context: String,
    },

    /// A footnote or endnote reference id had no matching note definition.
    MissingNoteContent {
        /// The note id.
        id: i32,
        /// Whether this was a footnote or endnote.
        kind: NoteKind,
    },

    /// A numbering id referenced by a paragraph had no matching definition.
    UnresolvedNumberingId {
        /// The unresolved `w:numId` value.
        num_id: u32,
    },

    /// A field instruction string was not recognised and stored as `Raw`.
    UnrecognisedField {
        /// The raw instruction string.
        instruction: String,
    },

    /// An image relationship could not be resolved.
    UnresolvedImage {
        /// The relationship id that could not be resolved.
        rel_id: String,
    },
}
