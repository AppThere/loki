// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 AppThere Loki contributors

//! `libxml2` (`xmllint`) backend for [`SchemaValidator`](super::SchemaValidator).
//!
//! Shells to `xmllint --noout {--schema|--relaxng} <schema> <xml>` and maps the
//! exit code + stderr to a [`SchemaReport`]. A pure-Rust XSD/RNG validator is
//! scarce, so this is the pragmatic shipping backend; the trait keeps consumers
//! insulated from it (Spec 02 §5).

use std::path::Path;
use std::process::Command;

use super::{SchemaError, SchemaKind, SchemaReport, SchemaValidator, SchemaViolation};

/// Exit codes for which `xmllint` reports a *document* problem (not-well-formed
/// or schema-invalid) rather than a harness/schema-setup failure. Anything else
/// nonzero is surfaced as [`SchemaError::Unexpected`].
const DOCUMENT_INVALID_CODES: &[i32] = &[1, 3, 4];

/// A [`SchemaValidator`] backed by the `xmllint` command-line tool.
#[derive(Clone, Debug)]
pub struct XmllintValidator {
    program: String,
}

impl XmllintValidator {
    /// Creates a validator, verifying `xmllint` is runnable. Returns
    /// [`SchemaError::XmllintNotFound`] if it is absent — the harness fails
    /// loudly rather than skipping validation.
    pub fn new() -> Result<Self, SchemaError> {
        Self::with_program("xmllint")
    }

    /// Like [`Self::new`] but with an explicit binary name/path (for testing or
    /// a non-standard install).
    pub fn with_program(program: impl Into<String>) -> Result<Self, SchemaError> {
        let program = program.into();
        match Command::new(&program).arg("--version").output() {
            Ok(out) if out.status.success() => Ok(Self { program }),
            Ok(out) => Err(SchemaError::Unexpected {
                code: out.status.code(),
                stderr: String::from_utf8_lossy(&out.stderr).into_owned(),
            }),
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => Err(SchemaError::XmllintNotFound),
            Err(e) => Err(SchemaError::Spawn(e)),
        }
    }

    /// Whether `xmllint` is available on `PATH`.
    #[must_use]
    pub fn is_available() -> bool {
        Self::new().is_ok()
    }
}

impl SchemaValidator for XmllintValidator {
    fn validate_file(
        &self,
        xml: &Path,
        schema: &Path,
        kind: SchemaKind,
    ) -> Result<SchemaReport, SchemaError> {
        let output = Command::new(&self.program)
            .arg("--noout")
            .arg(kind.xmllint_flag())
            .arg(schema)
            .arg(xml)
            .output()
            .map_err(|e| {
                if e.kind() == std::io::ErrorKind::NotFound {
                    SchemaError::XmllintNotFound
                } else {
                    SchemaError::Spawn(e)
                }
            })?;

        if output.status.success() {
            return Ok(SchemaReport::valid());
        }

        let stderr = String::from_utf8_lossy(&output.stderr);
        let code = output.status.code();
        if !code.is_some_and(|c| DOCUMENT_INVALID_CODES.contains(&c)) {
            return Err(SchemaError::Unexpected {
                code,
                stderr: stderr.into_owned(),
            });
        }

        let violations = parse_violations(&stderr, &xml.to_string_lossy());
        Ok(SchemaReport {
            valid: false,
            violations: if violations.is_empty() {
                // Nonzero exit with no parseable lines — still a failure.
                vec![SchemaViolation {
                    line: None,
                    message: stderr.trim().to_string(),
                }]
            } else {
                violations
            },
        })
    }
}

/// Parses `xmllint` stderr into [`SchemaViolation`]s. Error lines are prefixed
/// with `<xml-path>:<line>:`; continuation lines (no such prefix) extend the
/// previous message; the trailing `<xml-path> fails to validate` summary and
/// blank lines are dropped.
fn parse_violations(stderr: &str, xml_path: &str) -> Vec<SchemaViolation> {
    let prefix = format!("{xml_path}:");
    let mut out: Vec<SchemaViolation> = Vec::new();
    for raw in stderr.lines() {
        let line = raw.trim_end();
        if line.is_empty() || line == format!("{xml_path} fails to validate") {
            continue;
        }
        if let Some(rest) = line.strip_prefix(&prefix) {
            // rest = "<line>: <message>"
            let (lineno, message) = match rest.split_once(':') {
                Some((n, msg)) => (n.trim().parse::<u32>().ok(), msg.trim().to_string()),
                None => (None, rest.trim().to_string()),
            };
            out.push(SchemaViolation {
                line: lineno,
                message,
            });
        } else if let Some(last) = out.last_mut() {
            last.message.push(' ');
            last.message.push_str(line.trim());
        } else {
            out.push(SchemaViolation {
                line: None,
                message: line.trim().to_string(),
            });
        }
    }
    out
}

#[cfg(test)]
#[path = "xmllint_tests.rs"]
mod tests;
