// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 AppThere Loki contributors

//! Test-case severity, mirroring the master test plan.

use serde::{Deserialize, Serialize};

/// Severity of a fidelity divergence, per `TEST_PLAN.md`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
pub enum Severity {
    /// Silent data/layout corruption a reader notices immediately (wrong merge,
    /// dropped text, wrong page count, garbled glyphs).
    P0,
    /// Visible fidelity gap (wrong spacing, colour, wrap) a careful reader
    /// catches.
    P1,
    /// Subtle metric / typographic drift.
    P2,
}

impl Severity {
    /// Short uppercase label (`"P0"`, `"P1"`, `"P2"`).
    #[must_use]
    pub fn label(self) -> &'static str {
        match self {
            Severity::P0 => "P0",
            Severity::P1 => "P1",
            Severity::P2 => "P2",
        }
    }
}
