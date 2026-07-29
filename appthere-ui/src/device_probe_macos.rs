// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 AppThere Loki contributors

//! The memory probe on platforms with no `/proc/meminfo` — macOS and Windows
//! (Spec 08 I-25). Split from `device_probe.rs` at the 300-line ceiling.
//!
//! # Why this reports *available* now, when r53 deliberately did not
//!
//! macOS shipped total-only, on the reasoning that a guessed available is worse
//! than none: the derivation *prefers* available over total, so a number that
//! does not mean what the design assumes drives the budget confidently wrong.
//! That reasoning survives — it is why `vm_stat` was never summed by hand. What
//! it did not survive is r55's counter: over machine size × display scale × page
//! size × load, **total-only changes 26 of 72 regime cells**, and the error runs
//! the dangerous way on a loaded machine — firing later or never, which means
//! mounting at full scale against a ceiling derived from RAM the machine does not
//! have.
//!
//! So total-only is a correct *failure* and an inadequate *destination*, and the
//! answer is a real available figure rather than a guessed one.
//!
//! # `sysinfo` rather than an `unsafe` exception, decided on a stated standard
//!
//! Reaching `host_statistics64` / `GlobalMemoryStatusEx` directly needs `unsafe`,
//! and this crate carries `#![forbid(unsafe_code)]`. The alternative was a
//! documented exception in a dedicated crate — the shape the three Android
//! `cdylib`s already use.
//!
//! The tiebreak is that **one rule is written down and the other is not**.
//! `#![forbid(unsafe_code)]` is in CLAUDE.md and enforced by
//! `scripts/check-unsafe-policy.py`; "take no new dependencies" is a preference
//! this program has never stated. Trading the written rule for the unwritten one
//! needed a strong reason, and measurement removed it: in the configuration
//! actually shipped this is **one crate** whose whole transitive closure —
//! `libc`, `memchr` — is already in the lockfile at the same versions.
//!
//! The exception route would also have cost **two** exceptions in two FFI
//! surfaces, since macOS and Windows are both open. What it kept was the
//! zero-dependency principle, which is not a principle this program holds.
//!
//! # Linux and Android are untouched
//!
//! The dependency is target-scoped, so those builds gain nothing and lose
//! nothing, and the hand-parsed `MemAvailable` path the budget divisors are
//! calibrated against stays exactly as it was. Unifying all three on `sysinfo`
//! later would be semantics-preserving — its Linux `available_memory` *is*
//! `MemAvailable` — but it would change a working, calibrated path to gain
//! tidiness, and the platform it would change is the only one this environment
//! can test.
//!
//! # Verified by type-check, not by running
//!
//! Neither target can be executed here. Both are checked with `cargo check
//! --target aarch64-apple-darwin` and `--target x86_64-pc-windows-msvc`, which
//! catches signature and `cfg` errors and **does not** establish that the figures
//! are right on device. That is what the first run on each platform is for, and
//! `available_permille_of_total` is the line to read when it happens.

/// Total and available physical memory, or `None` for either figure the platform
/// declines to give.
///
/// Returns a pair rather than a struct so the caller keeps ownership of what
/// "unknown" means — the same reason `SystemMemory` carries two independent
/// options.
#[cfg(any(target_os = "macos", target_os = "windows"))]
pub(super) fn sysinfo_memory() -> (Option<u64>, Option<u64>) {
    use sysinfo::{MemoryRefreshKind, RefreshKind, System};

    // Only the RAM figures are refreshed: `System::new_all` would enumerate
    // processes, which is both slow and far more than a 5-second memory
    // resample needs.
    let sys = System::new_with_specifics(
        RefreshKind::nothing().with_memory(MemoryRefreshKind::nothing().with_ram()),
    );
    let total = sys.total_memory();
    let available = sys.available_memory();
    // Zero is not a reading on either figure: it would look like a successful
    // probe and drive the budget to its floor, which is the trap `parse_meminfo`
    // avoids by leaving a missing field `None` rather than defaulting it.
    (
        (total > 0).then_some(total),
        (available > 0).then_some(available),
    )
}

/// Parses `sysctl -n hw.memsize` output: a bare byte count.
///
/// Kept after the move to `sysinfo` because it is the one part of this module
/// testable on every platform, and because `hw.memsize` remains the exact,
/// static answer for total on macOS should the crate's figure ever be doubted.
/// Zero is rejected: a machine with no RAM is not a reading.
#[must_use]
pub fn parse_memsize(text: &str) -> Option<u64> {
    match text.trim().parse::<u64>() {
        Ok(0) | Err(_) => None,
        Ok(bytes) => Some(bytes),
    }
}
