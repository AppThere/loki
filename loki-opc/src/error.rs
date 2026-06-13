// Copyright 2026 AppThere Loki contributors
// SPDX-License-Identifier: MIT

//! Errors generated during OPC extraction, parsing, and serialization.

use thiserror::Error;

/// All errors produced by `loki-opc`.
#[non_exhaustive]
#[derive(Debug, Error)]
pub enum OpcError {
    /// A part name violates the OPC ABNF grammar (§6.2.2).
    #[error("invalid part name {name:?}: {reason}")]
    InvalidPartName {
        /// The offending part name.
        name: String,
        /// The violation encountered.
        reason: &'static str,
    },

    /// `[Content_Types].xml` could not be parsed.
    #[error("invalid content types XML: {0}")]
    InvalidContentTypes(String),

    /// A relationships part is malformed (missing id, bad target, or a
    /// duplicate id).
    #[error("invalid relationships XML in {part:?}: {reason}")]
    InvalidRelationships {
        /// The relationships part that failed to parse.
        part: String,
        /// Human-readable description of the problem.
        reason: String,
    },

    /// Two part names collide case-insensitively (only raised under `strict`).
    #[error("duplicate part name (case-insensitive collision): {0:?} and {1:?}")]
    DuplicatePartName(String, String),

    /// Error from the underlying ZIP container.
    #[error("ZIP error: {0}")]
    Zip(#[from] zip::result::ZipError),

    /// Error from the `quick_xml` parser.
    #[error("XML parse error: {0}")]
    Xml(#[from] quick_xml::Error),

    /// Underlying I/O failure.
    #[error("I/O error: {0}")]
    Io(#[from] std::io::Error),

    /// A ZIP entry uses a compression method other than stored or deflate.
    #[error("unsupported ZIP compression method: {0}")]
    UnsupportedCompression(String),

    /// The archive has no `[Content_Types].xml` and is not a valid OPC package.
    #[error("missing [Content_Types].xml — not a valid OPC package")]
    MissingContentTypes,

    /// More than one core-properties part is present; the spec allows at most
    /// one (§8.2).
    #[error("multiple Core Properties parts found — spec allows at most one (§8.2)")]
    MultipleCorePropsParts,

    /// A core-properties date/time field is not valid ISO 8601 / W3CDTF.
    #[error("date/time parse error in core properties: {0}")]
    DateTimeParse(String),

    /// No content type could be resolved for a part.
    #[error("unknown media type for part {part:?} with extension {extension:?}")]
    UnknownMediaType {
        /// The part with no resolvable media type.
        part: String,
        /// The part's file extension.
        extension: String,
    },

    /// The package uses digital signatures, which this version cannot read or
    /// write (§10).
    #[error("digital signatures are not supported in loki-opc v0.1.0 (§10)")]
    DigitalSignaturesNotSupported,

    /// A lookup referenced a part name that is not in the package.
    #[error("part not found: {0:?}")]
    PartNotFound(String),

    /// A single ZIP entry inflated past the per-entry decompression budget (zip-bomb guard).
    #[error("ZIP entry {name:?} exceeds the per-entry decompressed size limit of {limit} bytes")]
    EntryTooLarge {
        /// Name of the offending ZIP entry.
        name: String,
        /// The per-entry budget, in bytes, that was exceeded.
        limit: u64,
    },

    /// The aggregate decompressed size of all entries passed the package budget (zip-bomb guard).
    #[error("package exceeds the total decompressed size limit of {limit} bytes")]
    PackageTooLarge {
        /// The aggregate budget, in bytes, that was exceeded.
        limit: u64,
    },
}

/// Result alias for fallible `loki-opc` operations.
pub type OpcResult<T> = Result<T, OpcError>;

/// A deviation from ISO/IEC 29500-2:2021 observed during package open.
/// These represent non-conformant input that was handled gracefully.
/// Inspect these to audit the quality of input files.
#[non_exhaustive]
#[derive(Debug, Clone)]
pub enum DeviationWarning {
    /// A ZIP entry used `\` as a path separator. Normalised to `/` per §7.3.3.
    BackslashInPartName {
        /// The original entry name containing the backslash.
        original: String,
    },

    /// A part name used non-canonical percent-encoding. Re-encoded to canonical form per §7.3.4.
    NonCanonicalPercentEncoding {
        /// The original, non-canonical part name.
        original: String,
        /// The canonicalised part name that replaced it.
        normalised: String,
    },

    /// A part had a missing or empty media type in `[Content_Types].xml`. A fallback was applied per §6.2.3.
    MissingMediaType {
        /// The part with no declared media type.
        part: String,
        /// The fallback media type that was applied.
        fallback: String,
    },

    /// Two part names were equivalent under §6.3.5 case-folding. The first was retained.
    DuplicatePartName {
        /// The part name that was kept.
        retained: String,
        /// The colliding part name that was dropped.
        discarded: String,
    },

    /// Two relationships in one `.rels` file had the same `Id`. The first was retained per §6.5.3.
    DuplicateRelationshipId {
        /// The duplicated relationship id.
        id: String,
        /// The relationships part the duplicate was found in.
        part: String,
    },

    /// `[Content_Types].xml` was not found at the root. A case-insensitive fallback match was used.
    ContentTypesNotAtRoot {
        /// The path where the content-types part was actually found.
        found_at: String,
    },
}
