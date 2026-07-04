// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 AppThere Loki contributors

//! CPU/GPU render-parity cadence (Spec 06 M6 / §12, decision D5).
//!
//! Spec 02 renders conformance goldens on `vello_cpu` while the app paints on GPU
//! `vello`; the two pinned crates can drift as they version forward, so the parity
//! check (same scene both ways, expected to agree within tolerance) must run **on
//! every Vello version bump** and on a regular local cadence, on GPU hardware.
//!
//! The parity *check itself* needs a GPU and Spec 02's `vello_cpu` render path
//! (audit BM-3), so it runs on-device. This module makes the **version-bump
//! trigger** mechanical and headless: it compares the currently pinned Vello
//! version (from `Cargo.lock`) against the version the parity check was last
//! *confirmed* against (a committed marker), so a bump is a detectable, actionable
//! signal — not something to remember.

/// Whether the CPU/GPU parity check is due to run.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ParityStatus {
    /// The pinned Vello version matches the last confirmed parity run.
    UpToDate,
    /// The Vello version changed since the last confirmed run — re-run on GPU.
    Due {
        /// The version the check was last confirmed against.
        last: String,
        /// The currently pinned version.
        current: String,
    },
    /// No confirmed run is on record yet (marker missing/empty).
    NeverRun {
        /// The currently pinned version.
        current: String,
    },
}

impl ParityStatus {
    /// `true` when the parity check should be run (bumped or never run).
    #[must_use]
    pub fn is_due(&self) -> bool {
        !matches!(self, ParityStatus::UpToDate)
    }
}

/// Compares the currently pinned Vello version to the last-confirmed marker.
#[must_use]
pub fn parity_status(current_vello: &str, confirmed: Option<&str>) -> ParityStatus {
    match confirmed {
        None => ParityStatus::NeverRun {
            current: current_vello.to_string(),
        },
        Some(c) if c == current_vello => ParityStatus::UpToDate,
        Some(c) => ParityStatus::Due {
            last: c.to_string(),
            current: current_vello.to_string(),
        },
    }
}

/// Extracts the exact `vello` crate version from `Cargo.lock` text — the version
/// of the `[[package]]` whose `name = "vello"` (never `vello_cpu` etc.).
#[must_use]
pub fn vello_version_from_lock(cargo_lock: &str) -> Option<String> {
    let mut in_vello = false;
    for raw in cargo_lock.lines() {
        let line = raw.trim();
        if line == "[[package]]" {
            in_vello = false;
        } else if line == "name = \"vello\"" {
            in_vello = true;
        } else if in_vello && let Some(v) = line.strip_prefix("version = ") {
            return Some(v.trim_matches('"').to_string());
        }
    }
    None
}

/// Reads the confirmed Vello version from a parity marker file's text (the first
/// non-comment `vello_version <ver>` line). Comment (`#`) and blank lines ignored.
#[must_use]
pub fn confirmed_version_from_marker(marker: &str) -> Option<String> {
    for raw in marker.lines() {
        let line = raw.trim();
        if line.is_empty() || line.starts_with('#') {
            continue;
        }
        if let Some(rest) = line.strip_prefix("vello_version") {
            return rest.split_whitespace().next().map(str::to_string);
        }
    }
    None
}

/// Renders a parity marker recording `version` as the confirmed Vello version.
#[must_use]
pub fn render_marker(version: &str) -> String {
    format!(
        "# loki-bench CPU/GPU parity cadence marker (Spec 06 M6 / §12).\n\
         # The Vello version the parity check was last CONFIRMED against on GPU\n\
         # hardware. Update ONLY after a successful on-device parity run\n\
         # (see docs/adr/spec-06-discipline.md); note the date/device/tolerance below.\n\
         vello_version {version}\n\
         # last confirmed: <date, device, tolerance> — maintained by hand on commit\n"
    )
}

#[cfg(test)]
#[path = "parity_tests.rs"]
mod tests;
