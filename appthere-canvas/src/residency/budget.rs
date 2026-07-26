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

/// Share of *available* RAM at which the renderer stops protecting resolution
/// and starts protecting the process: one eighth.
///
/// This is **not** the budget. It is the line past which continuing to honour
/// full-resolution visible pages risks the OS killing us, and a soft page beats
/// a dead application. See [`TextureBudget::hard_ceiling_bytes`] for why the two
/// thresholds exist and why only this one may degrade what the user is reading.
///
/// # Why an eighth and not a quarter
///
/// Both were measured. A quarter never softens an ordinary operating point on any
/// device, but it permits a **946.7 MiB** peak on the 8 GB design floor (400% zoom
/// on a 3x display, two pages straddling a boundary), and a gigabyte of texture in
/// a document viewer is not defensible next to Spec 09's layout residency on the
/// same machine.
///
/// An eighth caps that case at 512 MiB and still clears every ordinary point on
/// every device with headroom — the worst ordinary case is 236.7 MiB at 200% on a
/// 3x display, against a 512 MiB ceiling on the design floor.
///
/// It does bind earlier on a genuinely memory-poor device: a phone reporting under
/// ~1.9 GiB available has a ceiling below that 236.7 MiB, so 200% on a 3x display
/// softens there. **That is the regime working rather than a regression.** On a
/// device that cannot afford two full-resolution pages, refusing to degrade does
/// not buy sharp text — it buys an OOM kill, and Android's killer is neither slow
/// nor negotiable.
pub const SURVIVAL_AVAILABLE_RAM_DIVISOR: u64 = 8;

/// Share of *total* RAM used for the survival ceiling when available is missing:
/// one sixteenth. Agrees with [`SURVIVAL_AVAILABLE_RAM_DIVISOR`] at the design
/// floor by the same construction as [`TOTAL_RAM_DIVISOR`].
pub const SURVIVAL_TOTAL_RAM_DIVISOR: u64 = 16;

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
    hard_ceiling_bytes: u64,
    source: BudgetSource,
}

impl TextureBudget {
    /// The budget for a device nothing is known about.
    #[must_use]
    pub fn baseline() -> Self {
        Self {
            bytes: BUDGET_BASELINE_BYTES,
            // The baseline stands for the 8 GB design floor's ~4 GiB available,
            // so its ceiling is that machine's ceiling, by the same
            // construction that anchors the budget itself.
            hard_ceiling_bytes: (4 * 1024 * 1024 * 1024_u64) / SURVIVAL_AVAILABLE_RAM_DIVISOR,
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
        let bytes = bytes.max(BUDGET_FLOOR_BYTES);
        Self {
            bytes,
            // A person may raise the *target* as high as they like; the survival
            // ceiling is not theirs to lower, because it is about the OOM killer
            // rather than about preference. It is only ever raised to keep the
            // invariant ceiling >= target.
            hard_ceiling_bytes: bytes.max(Self::baseline().hard_ceiling_bytes),
            source: BudgetSource::UserOverride,
        }
    }

    /// Builds a budget with both thresholds explicit, for tests.
    #[must_use]
    pub fn exact_with_ceiling(bytes: u64, hard_ceiling_bytes: u64) -> Self {
        let bytes = bytes.max(BUDGET_FLOOR_BYTES);
        Self {
            bytes,
            hard_ceiling_bytes: hard_ceiling_bytes.max(bytes),
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
                hard_ceiling_bytes: BUDGET_FLOOR_BYTES,
                source: BudgetSource::NoGpuPaintPath,
            };
        }
        if let Some(available) = inputs.available_ram_bytes {
            let bytes = clamp(available / AVAILABLE_RAM_DIVISOR);
            return Self {
                bytes,
                hard_ceiling_bytes: (available / SURVIVAL_AVAILABLE_RAM_DIVISOR).max(bytes),
                source: BudgetSource::AvailableRam,
            };
        }
        if let Some(total) = inputs.total_ram_bytes {
            let bytes = clamp(total / TOTAL_RAM_DIVISOR);
            return Self {
                bytes,
                hard_ceiling_bytes: (total / SURVIVAL_TOTAL_RAM_DIVISOR).max(bytes),
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

    /// The line past which the renderer will reduce the scale of pages the user
    /// is **looking at**, because the alternative is the process being killed.
    ///
    /// Two thresholds rather than one, and the distinction is the whole of
    /// ADR L08-026:
    ///
    /// - [`Self::bytes`] is a **target**. Exceeding it is a reported outcome, not
    ///   a failure to correct. It buys memory back from work the user cannot
    ///   see — the pre-render margin — and it never spends legibility to do it.
    /// - This is a **survival ceiling**. Between the two, visible pages stay at
    ///   full resolution and the plan simply reports that it is over target.
    ///   Above it, there is no benign move left: refusing to degrade means an
    ///   allocation the device cannot satisfy.
    ///
    /// The byte budget is our invention and the reader's perception is not, so
    /// the target may never cash in the second for the first. Survival is the
    /// one case where it may, because there the alternative is not "slightly
    /// more memory" but a dead application.
    #[must_use]
    pub fn hard_ceiling_bytes(self) -> u64 {
        self.hard_ceiling_bytes
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
