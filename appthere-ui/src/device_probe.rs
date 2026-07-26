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
/// TODO(device-profile-memory): macOS (`sysctl hw.memsize` +
/// `host_statistics64`) and Windows (`GlobalMemoryStatusEx`) report nothing
/// today, so a budget derived from this collapses to its baseline there. That
/// is the correct failure — a guessed total would set a real budget from a
/// number nobody measured — but it does mean L08-011 is only *demonstrated* on
/// Linux and Android until those land.
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
    #[cfg(not(any(target_os = "linux", target_os = "android")))]
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
