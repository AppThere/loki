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

//! Error and warning types for ODF import/export operations.
//!
//! [`OdfError`] represents fatal failures; [`OdfWarning`] represents
//! non-fatal issues collected during import and returned alongside the
//! document.

/// Fatal error returned by ODF import and export operations.
///
/// Variants are `#[non_exhaustive]` to allow adding new error kinds without
/// breaking downstream crates.
#[non_exhaustive]
#[derive(Debug, thiserror::Error)]
pub enum OdfError {
    /// A ZIP-level error from the `zip` crate.
    #[error("ZIP error: {0}")]
    Zip(#[from] zip::result::ZipError),

    /// An XML parse error in a specific ODF part.
    ///
    /// The `part` field names the ZIP entry (e.g. `"content.xml"`).
    #[error("XML parse error in {part:?}: {source}")]
    Xml {
        /// The ZIP entry name where the error occurred.
        part: String,
        /// The underlying quick-xml error.
        #[source]
        source: quick_xml::Error,
    },

    /// A required ODF part is absent from the package.
    ///
    /// ODF 1.3 §3.3 lists mandatory package entries.
    #[error("missing required part: {part}")]
    MissingPart {
        /// The ZIP entry name that was expected but not found.
        part: String,
    },

    /// An XML element is malformed or contains unexpected content.
    #[error("malformed {element} in {part:?}: {reason}")]
    MalformedElement {
        /// Local element name (e.g. `"office:document-content"`).
        element: String,
        /// ZIP entry where the element was found.
        part: String,
        /// Human-readable explanation.
        reason: String,
    },

    /// The `office:version` attribute holds an unrecognised value and
    /// `OdtImportOptions::strict_version` is `true`.
    ///
    /// ODF 1.3 §3 (office:version attribute).
    #[error("unsupported ODF version: {version:?}")]
    UnsupportedVersion {
        /// The raw version string from the document.
        version: String,
    },

    /// The requested feature has not yet been implemented.
    #[error("ODF import is not yet implemented for this feature: {feature}")]
    NotImplemented {
        /// Short description of the unimplemented feature.
        feature: String,
    },

    /// An I/O error from reading or writing the package stream.
    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),

    /// Failed to parse an integer attribute value.
    #[error("integer parse error: {0}")]
    ParseInt(#[from] std::num::ParseIntError),

    /// Failed to parse a floating-point attribute value.
    #[error("float parse error: {0}")]
    ParseFloat(#[from] std::num::ParseFloatError),
}

/// Convenience alias for `Result<T, OdfError>`.
pub type OdfResult<T> = Result<T, OdfError>;

/// Non-fatal warning collected during ODF import.
///
/// Warnings are accumulated in [`crate::odt::import::OdtImportResult::warnings`]
/// and returned to the caller alongside the document. They do not abort
/// processing.
#[non_exhaustive]
#[derive(Debug, Clone)]
pub enum OdfWarning {
    /// An XML element was encountered that this importer does not recognise.
    UnrecognisedElement {
        /// Local element name.
        element: String,
        /// Human-readable description of where the element appeared.
        context: String,
    },

    /// The `office:version` attribute held an unrecognised value.
    ///
    /// The importer treated it as the latest supported version.
    UnrecognisedVersion {
        /// The raw version string.
        version: String,
    },

    /// An image referenced by the document could not be found in the package.
    MissingImage {
        /// The `xlink:href` value of the missing image.
        href: String,
    },

    /// A `text:list-style-name` reference could not be resolved.
    UnresolvedListStyle {
        /// The style name that was referenced.
        name: String,
    },

    /// A foreign or extension element was preserved verbatim without being
    /// interpreted.
    PreservedForeignElement {
        /// Local element name of the preserved element.
        element: String,
    },
}
