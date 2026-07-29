// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 AppThere Loki contributors

//! Probes that fill in [`crate::DeviceProfile`] (Spec 08 T1.6, T2.0).
//!
//! # Probes push; the profile never pulls
//!
//! [`DeviceProfile`](crate::DeviceProfile) is a passive snapshot — see its
//! "Injectable" note. Everything here is a `note_*` function that folds an
//! observation into the ambient profile, matching
//! [`note_pointer`](crate::note_pointer). That is what lets a phase be tested
//! against a synthetic profile with no hardware (R12), and it is why a probe
//! that cannot answer writes nothing rather than writing a guess.
//!
//! # Landed with their consumers, not ahead of them
//!
//! r5 distributed the remaining T1.6 probes to the phases that consume them,
//! because a probe with no consumer cannot be tested and is exactly how
//! `Unknown` sat in the tree for three phases while L08-011 was asserted to be
//! true (R24). This module holds the **memory and GPU** probes, whose consumer
//! is Phase 2's texture budget.
//!
//! # `cfg(target_os)` here is API selection, not behaviour
//!
//! [`probe_system_memory`] reads `/proc/meminfo` where that file is the
//! platform's answer and reports nothing elsewhere. L08-011 forbids gating
//! *behaviour* on the compile target; picking the API that answers a question
//! on this platform is explicitly fine, and the resulting *value* — not the
//! target — is what any consumer branches on. An Android laptop with 16 GB and
//! a desktop with 16 GB read identically here, from different binaries.

use crate::device_profile::{AtDeviceProfileContext, GpuClass};
use dioxus::prelude::*;

/// What a memory probe found. Both fields are independently optional: a
/// platform may report a total without a usable "available" figure.
#[derive(Clone, Copy, PartialEq, Eq, Debug, Default)]
pub struct SystemMemory {
    /// Total physical RAM in bytes.
    pub total_bytes: Option<u64>,
    /// RAM the OS believes is available for a new allocation without swapping,
    /// in bytes.
    ///
    /// This is the figure a budget should prefer where it exists: total RAM
    /// says what the machine has, not what this process may take while a
    /// browser and the OS are also resident — which is the situation Spec 06's
    /// 8 GB design floor describes.
    pub available_bytes: Option<u64>,
}

impl SystemMemory {
    /// `true` when the probe answered nothing at all.
    #[must_use]
    pub fn is_unknown(self) -> bool {
        self.total_bytes.is_none() && self.available_bytes.is_none()
    }
}

/// Reads the platform's memory figures, or an empty [`SystemMemory`] where no
/// probe is implemented.
///
/// # macOS reports a total and, for now, no *available* figure
///
/// `hw.memsize` is exact and static, so it is read once. There is no macOS
/// equivalent of `MemAvailable` reachable without FFI: the OS compresses memory,
/// and the candidate sum from `vm_stat` (free + inactive + speculative +
/// purgeable) is an estimate whose relationship to "what this process may take"
/// is not the one Linux's field states. **A guessed *available* is worse than
/// none** — the derivation prefers it over total, so a number that does not mean
/// what the design assumes would drive the budget confidently wrong.
///
/// **That reasoning still holds and it is no longer sufficient (Spec 08 r55).**
/// A regime sweep measured what total-only costs: over machine size × display
/// scale × page size × load, **26 of 72 cells change the zoom at which the
/// survival regime fires**, and the error is asymmetric in the direction that
/// matters. On a *loaded* machine — 35% available, the case the whole
/// available-RAM design exists for — total-only fires **later or never**, which
/// means mounting at full scale against a ceiling derived from RAM the machine
/// does not have. The sharpest cell is 8 GiB at 2× on US Letter: available-based
/// fires at 375%, total-only never.
///
/// So the honest reading is that total-only is a correct *failure* and an
/// inadequate *destination*. macOS is in the same position as Windows, not a
/// solved case: both need a real available figure — `host_statistics64` here,
/// `GlobalMemoryStatusEx` there — and both routes need either a dependency or a
/// documented `unsafe` exception, which this crate forbids.
///
/// TODO(device-profile-memory): macOS `host_statistics64` and Windows
/// `GlobalMemoryStatusEx`. **Two platforms, two FFI surfaces, two exceptions —
/// or one `sysinfo` dependency covering all three.** The comparison is not the
/// one r54 stated: the exception route's cost doubles once macOS is counted with
/// Windows, while a single safe-API dependency stays flat. Until then L08-011 is
/// demonstrated on Linux and Android only, and macOS/Windows run on a budget
/// derived from total.
#[must_use]
pub fn probe_system_memory() -> SystemMemory {
    #[cfg(any(target_os = "linux", target_os = "android"))]
    {
        match std::fs::read_to_string("/proc/meminfo") {
            Ok(text) => parse_meminfo(&text),
            // Deliberately silent: this crate carries no logging dependency,
            // and an unreadable /proc/meminfo is a normal state on a platform
            // without one rather than something to report. The consumer sees
            // `Unknown` and falls back, which is the designed behaviour.
            Err(_) => SystemMemory::default(),
        }
    }
    #[cfg(target_os = "macos")]
    {
        SystemMemory {
            total_bytes: macos_total_bytes(),
            available_bytes: None,
        }
    }
    #[cfg(not(any(target_os = "linux", target_os = "android", target_os = "macos")))]
    {
        SystemMemory::default()
    }
}

/// Parses `MemTotal` and `MemAvailable` out of `/proc/meminfo` content.
///
/// Split from the file read so it is tested without a filesystem, on every
/// platform, against text captured from real machines. A field that is missing
/// or unparseable stays `None` rather than defaulting to zero: zero available
/// RAM is a meaningful value that would drive a budget straight to its floor.
#[must_use]
pub fn parse_meminfo(text: &str) -> SystemMemory {
    fn field(text: &str, key: &str) -> Option<u64> {
        // Matched with the colon attached, so `MemAvailable` cannot be
        // satisfied by a longer field that merely starts the same way.
        let prefix = format!("{key}:");
        let line = text.lines().find(|l| l.starts_with(&prefix))?;
        let rest = line.get(prefix.len()..)?.trim();
        let mut parts = rest.split_whitespace();
        let value: u64 = parts.next()?.parse().ok()?;
        // The kernel writes kB (which is really KiB) for these fields; a unit
        // we do not recognise is not silently treated as bytes.
        match parts.next() {
            Some("kB") | Some("KB") => value.checked_mul(1024),
            None => Some(value),
            Some(_) => None,
        }
    }
    SystemMemory {
        total_bytes: field(text, "MemTotal"),
        available_bytes: field(text, "MemAvailable"),
    }
}

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

/// Folds a memory observation into the ambient profile.
///
/// Writes only when something changed, so a probe re-run on resume does not
/// wake every consumer of the signal.
pub fn note_system_memory(observed: SystemMemory) {
    if observed.is_unknown() {
        return;
    }
    let Some(ctx) = try_consume_context::<AtDeviceProfileContext>() else {
        return;
    };
    let mut profile = ctx.profile;
    let current = *profile.peek();
    if current.system_ram_bytes == observed.total_bytes
        && current.available_ram_bytes == observed.available_bytes
    {
        return;
    }
    // The cross-platform assumption, collected rather than assumed (Spec 08
    // r56). `AVAILABLE_RAM_DIVISOR` is calibrated against Linux's
    // `MemAvailable`, which estimates reclaimable memory *including* page cache.
    // Windows' `ullAvailPhys` counts free plus standby; macOS's equivalent is
    // assembled from free, inactive and purgeable pages. Similar in intent, and
    // **not established** to report the same fraction of total under the same
    // load — so one divisor across three platforms is an assumption nothing has
    // measured.
    //
    // It is measurable the moment a platform reports both figures, which is
    // every platform that takes the available path at all. Logged here rather
    // than at the budget, because it is a property of the probe: the budget only
    // sees whichever figure arrived.
    if let (Some(total), Some(available)) = (observed.total_bytes, observed.available_bytes) {
        // `checked_div` rather than a guard: a zero total is not a reading, and
        // reporting 0 permille for one is the same statement as reporting
        // nothing while keeping the line's shape stable.
        let permille = available
            .saturating_mul(1000)
            .checked_div(total)
            .unwrap_or(0);
        tracing::debug!(
            total_bytes = total,
            available_bytes = available,
            available_permille_of_total = permille,
            "memory probe: available as a fraction of total",
        );
    }
    let mut w = profile.write();
    w.system_ram_bytes = observed.total_bytes;
    w.available_ram_bytes = observed.available_bytes;
}

/// Folds a GPU observation into the ambient profile.
///
/// [`GpuClass::Unknown`] is ignored rather than written: a probe that could not
/// answer must not overwrite one that did, which is the difference between "we
/// have not looked" and "we looked and there is nothing" (L9-009).
pub fn note_gpu_class(observed: GpuClass) {
    if observed == GpuClass::Unknown {
        return;
    }
    let Some(ctx) = try_consume_context::<AtDeviceProfileContext>() else {
        return;
    };
    let mut profile = ctx.profile;
    if profile.peek().gpu_class == observed {
        return;
    }
    profile.write().gpu_class = observed;
}

/// Folds an observed display scale factor into the ambient profile.
///
/// A non-finite or non-positive value is ignored on the same principle as
/// [`GpuClass::Unknown`] in [`note_gpu_class`]: a probe that could not answer
/// must not overwrite one that did.
///
/// Writes only on a change, so the common case — every frame after the first
/// observation on a stationary window — wakes nobody. The comparison is exact
/// rather than epsilon-based on purpose: compositors report scale factors as
/// exact values (1.0, 2.0, 1.5, 2.25), so any difference is a real display
/// change and worth a re-plan, and an epsilon would silently swallow the 2.0 →
/// 2.25 move between two Retina displays.
pub fn note_device_scale_factor(observed: f64) {
    // A forced scale factor wins: the first real paint would otherwise overwrite
    // it, and the 3x/4x rows this exists to reach would never be reached.
    if crate::device_profile_override::current()
        .device_scale_factor
        .is_some()
    {
        return;
    }
    if !observed.is_finite() || observed <= 0.0 {
        return;
    }
    let Some(ctx) = try_consume_context::<AtDeviceProfileContext>() else {
        return;
    };
    let mut profile = ctx.profile;
    if profile.peek().device_scale_factor == Some(observed) {
        return;
    }
    profile.write().device_scale_factor = Some(observed);
}

#[cfg(test)]
#[path = "device_probe_tests.rs"]
mod tests;
