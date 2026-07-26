// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 AppThere Loki contributors

//! The resident-texture byte budget (Spec 08 T2.1, ADR L08-002, ADR-0016).
//!
//! # Derived per device, never per compile target
//!
//! D-08 and L08-011: an Android laptop with 16 GB gets the same budget a
//! desktop with 16 GB gets, from a different binary. So the inputs here are
//! *measured quantities* — RAM, whether a GPU is painting — and never
//! `cfg!(target_os)`. The values arrive from `appthere_ui::DeviceProfile`, but
//! this module takes plain numbers rather than that type: `appthere-ui` is L5
//! and this crate is L4, so the application supplies the adaptation and the
//! arithmetic stays testable without a UI.

/// Smallest budget the derivation will produce, in bytes (24 MiB).
///
/// Below this a single visible page at moderate zoom cannot be held even at the
/// reduced-scale floor, so the budget would stop being a resolution dial and
/// start being a blank screen.
pub const BUDGET_FLOOR_BYTES: u64 = 24 * 1024 * 1024;

/// Budget used when nothing is known about the device (64 MiB).
///
/// Chosen so it is what the derivation *also* produces on Spec 06's 8 GB design
/// floor: an unknown machine is treated as the machine we designed for, rather
/// than as the best or worst case.
pub const BUDGET_BASELINE_BYTES: u64 = 64 * 1024 * 1024;

/// Largest budget the derivation will produce, in bytes (256 MiB).
///
/// This caps the *automatic* derivation on a large machine; it does not
/// overrule a person — see [`BudgetInputs::user_override_bytes`]. 256 MiB
/// clears the measured 200%-on-HiDPI worst case (157.8 MiB) with headroom.
pub const BUDGET_CEILING_BYTES: u64 = 256 * 1024 * 1024;

/// Share of *available* RAM the budget may take: one 64th.
///
/// Calibrated rather than picked: Spec 06's design floor is an 8 GB machine
/// also running a browser and an OS, which typically reports ~4 GiB available,
/// and 4 GiB / 64 is exactly [`BUDGET_BASELINE_BYTES`]. The whole scale is
/// anchored on that one point.
pub const AVAILABLE_RAM_DIVISOR: u64 = 64;

/// Share of *total* RAM used when the available figure is missing: one 128th.
///
/// Half the available-RAM share, because total overstates what this process may
/// take. The two paths agree at the design floor by construction — 8 GiB / 128
/// and 4 GiB / 64 are the same number — which is what makes the fallback a
/// degradation rather than a different policy.
pub const TOTAL_RAM_DIVISOR: u64 = 128;

/// What the derivation reads.
///
/// Every field is optional or defaults to the permissive answer, so a
/// `Default` value derives [`BUDGET_BASELINE_BYTES`] rather than the floor.
/// That direction matters: an input nobody filled in must not silently
/// throttle the renderer.
#[derive(Clone, Copy, PartialEq, Eq, Debug, Default)]
pub struct BudgetInputs {
    /// RAM the OS reports as available, from
    /// `appthere_ui::DeviceProfile::available_ram_bytes`.
    pub available_ram_bytes: Option<u64>,
    /// Total physical RAM, used only when the available figure is missing.
    pub total_ram_bytes: Option<u64>,
    /// Whether a GPU paint path is in use. `None` means not yet probed.
    ///
    /// This is the *only* way the GPU class enters the derivation, and it is
    /// worth being plain about why: wgpu exposes no portable VRAM figure, so
    /// there is nothing to derive a discrete GPU's budget from. A discrete card
    /// therefore gets the same system-RAM-derived budget an integrated one
    /// gets, which under-uses it. That is the safe direction and it is an
    /// **inference, not a measurement** — see
    /// `DeviceProfile::gpu_class`'s `TODO(device-profile-gpu-memory)`.
    pub gpu_paint_path: Option<bool>,
    /// An explicit user setting, in bytes.
    pub user_override_bytes: Option<u64>,
}

/// How a budget figure was arrived at. Reported so a surprising budget can be
/// explained without re-deriving it by hand.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum BudgetSource {
    /// An explicit user setting.
    UserOverride,
    /// A share of the OS-reported available RAM.
    AvailableRam,
    /// A share of total RAM, because available was not reported.
    TotalRam,
    /// Nothing was known about the device.
    Baseline,
    /// No GPU paint path, so no page textures are allocated at all.
    NoGpuPaintPath,
}

/// A resident-texture byte budget.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct TextureBudget {
    bytes: u64,
    source: BudgetSource,
}

impl TextureBudget {
    /// The budget for a device nothing is known about.
    #[must_use]
    pub fn baseline() -> Self {
        Self {
            bytes: BUDGET_BASELINE_BYTES,
            source: BudgetSource::Baseline,
        }
    }

    /// An explicit figure, for tests and for the user override path.
    ///
    /// Clamped up to [`BUDGET_FLOOR_BYTES`] but **not** down to the ceiling: the
    /// ceiling bounds an automatic derivation, not a person who knows their
    /// machine.
    #[must_use]
    pub fn exact(bytes: u64) -> Self {
        Self {
            bytes: bytes.max(BUDGET_FLOOR_BYTES),
            source: BudgetSource::UserOverride,
        }
    }

    /// Derives the budget from what is known about the device.
    #[must_use]
    pub fn derive(inputs: BudgetInputs) -> Self {
        if let Some(bytes) = inputs.user_override_bytes {
            return Self::exact(bytes);
        }
        // A device with no GPU paint path allocates no page textures, so this
        // arm never binds anything in practice; it is here so the budget
        // reported in a diagnostic matches the device rather than describing a
        // renderer that is not running.
        if inputs.gpu_paint_path == Some(false) {
            return Self {
                bytes: BUDGET_FLOOR_BYTES,
                source: BudgetSource::NoGpuPaintPath,
            };
        }
        if let Some(available) = inputs.available_ram_bytes {
            return Self {
                bytes: clamp(available / AVAILABLE_RAM_DIVISOR),
                source: BudgetSource::AvailableRam,
            };
        }
        if let Some(total) = inputs.total_ram_bytes {
            return Self {
                bytes: clamp(total / TOTAL_RAM_DIVISOR),
                source: BudgetSource::TotalRam,
            };
        }
        Self::baseline()
    }

    /// The budget in bytes.
    #[must_use]
    pub fn bytes(self) -> u64 {
        self.bytes
    }

    /// How the figure was arrived at.
    #[must_use]
    pub fn source(self) -> BudgetSource {
        self.source
    }
}

fn clamp(bytes: u64) -> u64 {
    bytes.clamp(BUDGET_FLOOR_BYTES, BUDGET_CEILING_BYTES)
}

#[cfg(test)]
#[path = "budget_tests.rs"]
mod tests;
