// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 AppThere Loki contributors

//! The macOS half of the memory probe, split from `device_probe.rs` at the
//! 300-line ceiling.
//!
//! Total only, and deliberately: see the parent module for why a guessed
//! *available* is worse than none here — and for the r55 counter showing that
//! total-only is a correct failure rather than an adequate destination.

/// Total physical RAM on macOS, from `sysctl -n hw.memsize`.
///
/// # Why a subprocess, and why exactly once
///
/// The native call is `sysctlbyname`, which is FFI, and this crate carries
/// `#![forbid(unsafe_code)]`. The alternatives were to relax that for one read,
/// to add a system-info dependency to a UI crate, or to shell out — and shelling
/// out is the smallest commitment of the three for a value that **cannot
/// change**: physical RAM is fixed for the life of the process, so this is
/// cached and the 5-second resample never spawns anything.
///
/// A failure — `sysctl` missing, output unparseable — yields `None` and the
/// budget falls back exactly as it did before, which is the behaviour this
/// replaces rather than a new risk.
#[cfg(target_os = "macos")]
fn macos_total_bytes() -> Option<u64> {
    use std::sync::OnceLock;
    static TOTAL: OnceLock<Option<u64>> = OnceLock::new();
    *TOTAL.get_or_init(|| {
        let out = std::process::Command::new("sysctl")
            .args(["-n", "hw.memsize"])
            .output()
            .ok()?;
        parse_memsize(&String::from_utf8_lossy(&out.stdout))
    })
}

/// Parses `sysctl -n hw.memsize` output: a bare byte count.
///
/// Split from the call so it is tested on every platform, like `parse_meminfo`.
/// Zero is rejected: a machine with no RAM is not a reading, and zero would
/// drive the budget to its floor while looking like a successful probe.
#[must_use]
pub fn parse_memsize(text: &str) -> Option<u64> {
    match text.trim().parse::<u64>() {
        Ok(0) | Err(_) => None,
        Ok(bytes) => Some(bytes),
    }
}
