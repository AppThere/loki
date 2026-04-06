// Copyright 2024-2026 AppThere
//
// Licensed under the MIT License.

//! Errors generated during OPC extraction, parsing, and serialization.

use thiserror::Error;

/// All errors produced by `loki-opc`.
#[non_exhaustive]
#[derive(Debug, Error)]
pub enum OpcError {
    /// Issued when a part name breaks the ABNF conventions defined in §6.2.2.
    #[error("invalid part name {name:?}: {reason}")]
    InvalidPartName {
        /// Attempted logical component address.
        name: String,
        /// The violation encountered.
        reason: &'static str,
    },

    /// Corrupt XML payload specifically mapping `[Content_Types].xml`.
    #[error("invalid content types XML: {0}")]
    InvalidContentTypes(String),

    /// Relationships missing required ID, targeting incorrect types, or breaking uniqueness.
    #[error("invalid relationships XML in {part:?}: {reason}")]
    InvalidRelationships {
        /// Relationships component file.
        part: String,
        /// Descriptive error payload.
        reason: String,
    },

    /// Thrown solely when `strict` forces checking for unique item maps failing case insensitivity rules.
    #[error("duplicate part name (case-insensitive collision): {0:?} and {1:?}")]
    DuplicatePartName(String, String),

    /// Passed through issues deriving from physical storage implementations.
    #[error("ZIP error: {0}")]
    Zip(#[from] zip::result::ZipError),

    /// General syntax issues derived through validation handlers and payload parsers via `quick_xml`.
    #[error("XML parse error: {0}")]
    Xml(#[from] quick_xml::Error),

    /// I/O operations failed during access across file descriptors.
    #[error("I/O error: {0}")]
    Io(#[from] std::io::Error),

    /// Blocked because ZIP implementations utilize LZMA, BZIP2, or other restricted compression values outside allowed standard.
    #[error("unsupported ZIP compression method: {0}")]
    UnsupportedCompression(String),

    /// Triggered initially upon package validation.
    #[error("missing [Content_Types].xml — not a valid OPC package")]
    MissingContentTypes,

    /// Multiple `.core-properties` XML types present despite explicit restriction dictating only one allowed globally per §8.2.
    #[error("multiple Core Properties parts found — spec allows at most one (§8.2)")]
    MultipleCorePropsParts,

    /// Chronological schema violations via ISO parsing.
    #[error("date/time parse error in core properties: {0}")]
    DateTimeParse(String),

    /// Encountered missing values during file conversion, bypassing explicit mappings.
    #[error("unknown media type for part {part:?} with extension {extension:?}")]
    UnknownMediaType {
        /// Failing component address.
        part: String,
        /// Corresponding native filesystem string.
        extension: String,
    },

    /// Refused handling access to digital signatures intentionally.
    #[error("digital signatures are not supported in loki-opc v0.1.0 (§10)")]
    DigitalSignaturesNotSupported,

    /// Indicated when a searched logical name cannot map to physical assets directly.
    #[error("part not found: {0:?}")]
    PartNotFound(String),
}

/// Convenience result mapped across internal IO parsing values and validations.
pub type OpcResult<T> = Result<T, OpcError>;

/// A deviation from ISO/IEC 29500-2:2021 observed during package open.
/// These represent non-conformant input that was handled gracefully.
/// Inspect these to audit the quality of input files.
#[non_exhaustive]
#[derive(Debug, Clone)]
pub enum DeviationWarning {
    /// A ZIP entry used `\` as a path separator. Normalised to `/` per §7.3.3.
    BackslashInPartName {
        /// Captured violation literal.
        original: String, 
    },

    /// A part name used non-canonical percent-encoding. Re-encoded to canonical form per §7.3.4.
    NonCanonicalPercentEncoding { 
        /// Escaped strings failing structural checks.
        original: String, 
        /// Fallback successfully rewritten format matching the original intent.
        normalised: String,
    },

    /// A part had a missing or empty media type in [Content_Types].xml. A fallback was applied per §6.2.3.
    MissingMediaType { 
        /// Part identifier mapped correctly but skipped locally mapping types natively.
        part: String, 
        /// Applied implicit substitution identifier correctly identifying the part intent.
        fallback: String,
    },

    /// Two part names were equivalent under §6.3.5 case-folding. The first was retained.
    DuplicatePartName { 
        /// Primary mapping retained matching its origin sequence.
        retained: String, 
        /// Secondary reference stripped from extraction output.
        discarded: String,
    },

    /// Two relationships in one `.rels` file had the same `Id`. The first was retained per §6.5.3.
    DuplicateRelationshipId { 
        /// The duplicated id literal parsed matching both parts uniformly.
        id: String, 
        /// Local package tracking mapping.
        part: String,
    },

    /// `[Content_Types].xml` was not found at the root. A case-insensitive fallback match was used.
    ContentTypesNotAtRoot { 
        /// String sequence where the structural marker was found correctly.
        found_at: String,
    },
}
