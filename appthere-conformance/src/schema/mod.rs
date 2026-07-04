// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 AppThere Loki contributors

//! Axis 1 — schema validation.
//!
//! Every export the harness produces is validated against the official schema
//! *before* any rendering question is asked: OOXML parts against the ISO/IEC
//! 29500 XSDs (Transitional by default, Strict opt-in) and the OPC package
//! layer; ODF against the OASIS RELAX NG schema and manifest.
//!
//! [`SchemaValidator`] is the backend-agnostic trait. The shipping
//! implementation ([`xmllint::XmllintValidator`]) shells to `libxml2`'s
//! `xmllint`; a future pure-Rust backend can replace it without touching
//! consumers. Validator availability is checked explicitly — a missing
//! `xmllint` fails loudly, never silently skips (Spec 02 §5).
//!
//! The schemas themselves are vendored and version-pinned under `schemas/`
//! (see `schemas/README.md`) so validation is reproducible and offline (D6).

use std::path::Path;

pub mod xmllint;

/// The schema language a validation runs against.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SchemaKind {
    /// W3C XML Schema (`.xsd`) — used for ISO/IEC 29500 (OOXML).
    Xsd,
    /// RELAX NG (`.rng`) — used for the OASIS ODF schema.
    RelaxNg,
}

impl SchemaKind {
    /// The `xmllint` flag selecting this schema language.
    #[must_use]
    pub fn xmllint_flag(self) -> &'static str {
        match self {
            SchemaKind::Xsd => "--schema",
            SchemaKind::RelaxNg => "--relaxng",
        }
    }
}

/// A single schema-validity error, located in the candidate document.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SchemaViolation {
    /// 1-based line in the candidate XML, if the validator reported one.
    pub line: Option<u32>,
    /// Human-readable description from the validator.
    pub message: String,
}

/// The outcome of validating one XML document against one schema.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SchemaReport {
    /// `true` iff the document is schema-valid (no violations).
    pub valid: bool,
    /// Each reported violation, in document order. Empty when `valid`.
    pub violations: Vec<SchemaViolation>,
}

impl SchemaReport {
    /// A passing report with no violations.
    #[must_use]
    pub fn valid() -> Self {
        Self {
            valid: true,
            violations: Vec::new(),
        }
    }
}

/// Errors that prevent a validation from *running* (distinct from the document
/// being invalid, which is a [`SchemaReport`] with `valid == false`).
#[derive(Debug, thiserror::Error)]
pub enum SchemaError {
    /// The `xmllint` binary was not found on `PATH`. The harness fails loudly
    /// rather than skipping validation silently (Spec 02 §5).
    #[error(
        "xmllint (libxml2) not found on PATH — install libxml2-utils; schema \
         validation must not be silently skipped"
    )]
    XmllintNotFound,
    /// Spawning or running `xmllint` failed for an environmental reason.
    #[error("failed to run xmllint: {0}")]
    Spawn(#[source] std::io::Error),
    /// Reading/writing the candidate or a temp file failed.
    #[error("schema validation I/O error: {0}")]
    Io(#[source] std::io::Error),
    /// `xmllint` exited with an unexpected status (neither valid nor the
    /// well-defined "invalid document" code), with its captured stderr.
    #[error("xmllint exited unexpectedly (code {code:?}): {stderr}")]
    Unexpected {
        /// The process exit code, if any.
        code: Option<i32>,
        /// Captured stderr for diagnosis.
        stderr: String,
    },
}

/// Validates serialized XML against a schema. Backend-agnostic so a pure-Rust
/// validator can later replace the libxml2 one without changing consumers.
pub trait SchemaValidator {
    /// Validates the XML file at `xml` against `schema` (of language `kind`).
    ///
    /// Returns `Ok(report)` whether or not the document is valid — an invalid
    /// document is `report.valid == false` with [`SchemaViolation`]s, not an
    /// `Err`. `Err` is reserved for failures that prevent validation from
    /// running (see [`SchemaError`]).
    fn validate_file(
        &self,
        xml: &Path,
        schema: &Path,
        kind: SchemaKind,
    ) -> Result<SchemaReport, SchemaError>;

    /// Validates in-memory XML `bytes` against `schema`. The default impl writes
    /// the bytes to a temp file and delegates to [`Self::validate_file`].
    fn validate_bytes(
        &self,
        bytes: &[u8],
        schema: &Path,
        kind: SchemaKind,
    ) -> Result<SchemaReport, SchemaError> {
        let dir = tempfile::tempdir().map_err(SchemaError::Io)?;
        let path = dir.path().join("candidate.xml");
        std::fs::write(&path, bytes).map_err(SchemaError::Io)?;
        self.validate_file(&path, schema, kind)
    }
}
