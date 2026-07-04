// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 AppThere Loki contributors

//! OS-level resident-set-size measurement (Spec 06 M5 / §10) — the device-bound
//! memory reality check that complements the portable allocation signal (D1).
//!
//! The *number* is machine-specific, but the Linux mechanism (`/proc/self/status`)
//! runs anywhere with `/proc`, so it is verifiable headless. The macOS / Windows
//! readers (`getrusage` `ru_maxrss` / `GetProcessMemoryInfo`) are the on-device
//! follow-up for Kevin's MacBook A16 and Windows box — they need platform FFI
//! this `forbid(unsafe_code)` crate leaves to the device build, and cannot be
//! exercised in the agent regardless.

/// Peak resident set size (high-water mark) of this process, in bytes.
///
/// `None` when the platform has no reader yet (currently everything but Linux).
#[must_use]
pub fn peak_rss_bytes() -> Option<u64> {
    read_status_kib("VmHWM").map(|kib| kib * 1024)
}

/// Current resident set size of this process, in bytes. `None` off Linux.
#[must_use]
pub fn current_rss_bytes() -> Option<u64> {
    read_status_kib("VmRSS").map(|kib| kib * 1024)
}

#[cfg(target_os = "linux")]
fn read_status_kib(key: &str) -> Option<u64> {
    let status = std::fs::read_to_string("/proc/self/status").ok()?;
    parse_status_kib(&status, key)
}

#[cfg(not(target_os = "linux"))]
fn read_status_kib(_key: &str) -> Option<u64> {
    None // macOS/Windows plumbing is the on-device task (Spec 06 M5 §10).
}

/// Parses a `/proc/self/status` line like `VmHWM:\t 1952 kB`, returning the KiB
/// value for `key`. Pure, so it is unit-tested without `/proc`.
#[must_use]
pub fn parse_status_kib(status: &str, key: &str) -> Option<u64> {
    for line in status.lines() {
        if let Some(rest) = line.strip_prefix(key) {
            let rest = rest.trim_start();
            if !rest.starts_with(':') {
                continue; // e.g. `key` is a prefix of a different field name
            }
            let value = rest.trim_start_matches(':').trim();
            return value.split_whitespace().next()?.parse::<u64>().ok();
        }
    }
    None
}

#[cfg(test)]
#[path = "rss_tests.rs"]
mod tests;
