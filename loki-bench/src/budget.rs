// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 AppThere Loki contributors

//! Per-tier peak-RSS budgets (Spec 06 M5 / §9, decision D2).
//!
//! A budget is a **review target, never a gate** (§11): a measurement over budget
//! prompts a look, not a build failure. Budgets are *calibrated* — set from
//! measured behaviour plus headroom against the 8 GB floor, not guessed — and
//! committed alongside the calibration record so they trace to data.

use std::collections::BTreeMap;

/// Whether a measured peak RSS is within its tier budget.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum BudgetStatus {
    /// At or under budget.
    WithinBudget,
    /// Over budget — review.
    OverBudget,
}

/// Compares a measured peak RSS (bytes) against a tier budget (bytes).
#[must_use]
pub fn check(measured_bytes: u64, budget_bytes: u64) -> BudgetStatus {
    if measured_bytes <= budget_bytes {
        BudgetStatus::WithinBudget
    } else {
        BudgetStatus::OverBudget
    }
}

/// Headroom remaining under budget as a fraction of the budget (negative when
/// over). `0.0` for a zero budget.
#[must_use]
pub fn headroom_frac(measured_bytes: u64, budget_bytes: u64) -> f64 {
    if budget_bytes == 0 {
        return 0.0;
    }
    (budget_bytes as f64 - measured_bytes as f64) / budget_bytes as f64
}

/// Committed per-tier peak-RSS budgets: tier → budget bytes.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Budgets {
    entries: BTreeMap<String, u64>,
}

impl Budgets {
    /// Builds budgets from `(tier, budget_bytes)` pairs.
    #[must_use]
    pub fn from_pairs(pairs: &[(String, u64)]) -> Self {
        Self {
            entries: pairs.iter().cloned().collect(),
        }
    }

    /// The budget for `tier`, if set.
    #[must_use]
    pub fn get(&self, tier: &str) -> Option<u64> {
        self.entries.get(tier).copied()
    }

    /// Renders a stable, comment-headed `tier  budget_bytes` file.
    #[must_use]
    pub fn render(&self) -> String {
        let mut out = String::new();
        out.push_str("# loki-bench per-tier peak-RSS budgets (Spec 06 M5 / §9, bytes).\n");
        out.push_str(
            "# Review targets, not gates. Recalibrate on device: see spec-06-calibration.md\n",
        );
        out.push_str("# tier  budget_bytes\n");
        for (tier, bytes) in &self.entries {
            out.push_str(&format!("{tier:<24} {bytes:>14}\n"));
        }
        out
    }

    /// Parses the `tier  budget_bytes` format, ignoring blank / `#` lines.
    ///
    /// # Errors
    /// Returns the 1-based line number of the first malformed line.
    pub fn parse(text: &str) -> Result<Self, usize> {
        let mut entries = BTreeMap::new();
        for (i, raw) in text.lines().enumerate() {
            let line = raw.trim();
            if line.is_empty() || line.starts_with('#') {
                continue;
            }
            let mut f = line.split_whitespace();
            let (Some(tier), Some(bytes), None) = (f.next(), f.next(), f.next()) else {
                return Err(i + 1);
            };
            let bytes: u64 = bytes.parse().map_err(|_| i + 1)?;
            entries.insert(tier.to_string(), bytes);
        }
        Ok(Self { entries })
    }
}

#[cfg(test)]
#[path = "budget_tests.rs"]
mod tests;
