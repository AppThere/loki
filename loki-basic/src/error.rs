// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 AppThere Loki contributors

//! Typed errors for every interpreter phase, plus source [`Span`]s.
//!
//! Lex and parse failures are static (bad program text); [`RuntimeError`]
//! models a *trappable* BASIC runtime error, carrying the classic VBA
//! `Err.Number` so `On Error` handlers and the `Err` object behave as authors
//! expect. Feature-refusals from the "never" list (macro spec §7) are a
//! distinct, **untrappable** runtime error number so a malicious macro cannot
//! `On Error Resume Next` its way past a refused capability.

use thiserror::Error;

/// A half-open byte range `[start, end)` into the original source text.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct Span {
    /// Inclusive start byte offset.
    pub start: usize,
    /// Exclusive end byte offset.
    pub end: usize,
}

impl Span {
    /// Creates a span from a start and end byte offset.
    #[must_use]
    pub fn new(start: usize, end: usize) -> Self {
        Self { start, end }
    }
}

/// The umbrella error for the whole pipeline (`lex → parse → run`).
#[derive(Debug, Error, Clone, PartialEq)]
pub enum BasicError {
    /// The source text could not be tokenised.
    #[error("lex error at {}..{}: {message}", span.start, span.end)]
    Lex {
        /// Human-readable reason.
        message: String,
        /// Location in the source.
        span: Span,
    },
    /// The token stream did not form a valid program.
    #[error("parse error at {}..{}: {message}", span.start, span.end)]
    Parse {
        /// Human-readable reason.
        message: String,
        /// Location in the source.
        span: Span,
    },
    /// A trappable (or refused) runtime error.
    #[error("runtime error {}: {}", .0.number, .0.message)]
    Runtime(RuntimeError),
}

/// A BASIC runtime error, modelled on VBA's `Err` object.
///
/// `number` is the classic error code (e.g. `13` type mismatch, `11` division
/// by zero). `trappable == false` marks a "never"-list feature refusal (spec
/// §7): `On Error` cannot swallow it.
#[derive(Debug, Error, Clone, PartialEq)]
#[error("error {number}: {message}")]
pub struct RuntimeError {
    /// VBA-compatible error number (`Err.Number`).
    pub number: i32,
    /// Error description (`Err.Description`).
    pub message: String,
    /// Whether an `On Error` handler may trap this error. Feature refusals and
    /// resource-limit stops are untrappable.
    pub trappable: bool,
    /// Source location, when known.
    pub span: Option<Span>,
}

impl RuntimeError {
    /// Builds a trappable runtime error with the given VBA number.
    #[must_use]
    pub fn new(number: i32, message: impl Into<String>) -> Self {
        Self {
            number,
            message: message.into(),
            trappable: true,
            span: None,
        }
    }

    /// Attaches a source span (builder).
    #[must_use]
    pub fn at(mut self, span: Span) -> Self {
        self.span = Some(span);
        self
    }

    /// Marks the error untrappable (feature refusal / resource limit).
    #[must_use]
    pub fn untrappable(mut self) -> Self {
        self.trappable = false;
        self
    }

    // ── Standard VBA runtime errors (the trappable compute set) ─────────────

    /// Error 6 — overflow.
    #[must_use]
    pub fn overflow() -> Self {
        Self::new(6, "Overflow")
    }

    /// Error 9 — subscript out of range.
    #[must_use]
    pub fn subscript_out_of_range() -> Self {
        Self::new(9, "Subscript out of range")
    }

    /// Error 11 — division by zero.
    #[must_use]
    pub fn division_by_zero() -> Self {
        Self::new(11, "Division by zero")
    }

    /// Error 13 — type mismatch.
    #[must_use]
    pub fn type_mismatch() -> Self {
        Self::new(13, "Type mismatch")
    }

    /// Error 5 — invalid procedure call or argument.
    #[must_use]
    pub fn invalid_call() -> Self {
        Self::new(5, "Invalid procedure call or argument")
    }

    /// Error 6 tier — a value that does not fit the target numeric range.
    #[must_use]
    pub fn out_of_range() -> Self {
        Self::new(6, "Overflow")
    }

    // ── Loki-specific untrappable stops ─────────────────────────────────────

    /// A "never"-list feature was invoked (spec §7). Untrappable and named so
    /// the author understands the refusal.
    #[must_use]
    pub fn feature_refused(feature: &str) -> Self {
        // Error number 1004 is a well-known "application-defined" code; we reuse
        // it with a distinctive message so the refusal is unmistakable.
        Self::new(
            1004,
            format!("Feature refused (disabled for safety): {feature}"),
        )
        .untrappable()
    }

    /// The fuel budget was exhausted (spec §8). Untrappable so a runaway macro
    /// cannot loop past the stop inside an error handler.
    #[must_use]
    pub fn fuel_exhausted() -> Self {
        Self::new(1005, "Macro stopped: resource budget exhausted").untrappable()
    }

    /// Execution was cancelled by the host (user pressed Stop; spec §8).
    #[must_use]
    pub fn cancelled() -> Self {
        Self::new(1006, "Macro cancelled").untrappable()
    }

    /// The internal `End`/`Stop` halt sentinel — unwinds all execution. Not a
    /// real error; the public entry point maps it to a clean stop.
    #[must_use]
    pub fn halt() -> Self {
        Self::new(i32::MIN, "halt").untrappable()
    }

    /// Whether this is the halt sentinel.
    #[must_use]
    pub fn is_halt(&self) -> bool {
        self.number == i32::MIN
    }
}

impl From<RuntimeError> for BasicError {
    fn from(e: RuntimeError) -> Self {
        BasicError::Runtime(e)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn standard_errors_carry_vba_numbers() {
        assert_eq!(RuntimeError::type_mismatch().number, 13);
        assert_eq!(RuntimeError::division_by_zero().number, 11);
        assert_eq!(RuntimeError::subscript_out_of_range().number, 9);
    }

    #[test]
    fn refusals_and_limits_are_untrappable() {
        assert!(!RuntimeError::feature_refused("Shell").trappable);
        assert!(!RuntimeError::fuel_exhausted().trappable);
        assert!(!RuntimeError::cancelled().trappable);
        assert!(RuntimeError::type_mismatch().trappable);
    }
}
