// SPDX-License-Identifier: Apache-2.0

//! The application's view of the resident page-texture budget (Spec 08 T2.1).
//!
//! # Why the join happens here
//!
//! The budget's *arithmetic* is `appthere_canvas::residency` (L4) and its
//! *inputs* are `appthere_ui::DeviceProfile` (L5), and L4 may not depend on L5.
//! `loki-text` is L6 and sees both, so it reads the profile, derives the
//! figure, and passes it down to `DocumentView` as a plain number — the same
//! shape `page_gap_px` already uses for the design tokens (Spec 01 audit A-8).
//!
//! # The user override
//!
//! T2.1 requires the budget to be user-overridable. It is, through
//! `LOKI_TEXTURE_BUDGET_MB`, read once per process.
//!
//! TODO(texture-budget-ui): surface this as a setting rather than an
//! environment variable. The variable is a real override and is documented, but
//! it is not discoverable, so "always user-overridable" is satisfied in
//! mechanism and not yet in reach. The settings surface arrives with Phase 5's
//! zoom and colour work, which is where a preferences panel first exists.

use appthere_canvas::residency::{BudgetInputs, TextureBudget};
use appthere_ui::{GpuClass, use_device_profile};

/// Environment variable holding a user budget override, in whole MiB.
pub const OVERRIDE_ENV: &str = "LOKI_TEXTURE_BUDGET_MB";

/// Reads the override once and caches the result.
///
/// Once per process rather than per frame: a budget that changed under the
/// renderer mid-session would re-plan every tile on a variable nobody expects
/// to be live.
fn override_bytes() -> Option<u64> {
    use std::sync::OnceLock;
    static CACHED: OnceLock<Option<u64>> = OnceLock::new();
    *CACHED.get_or_init(|| {
        let raw = std::env::var(OVERRIDE_ENV).ok()?;
        let mib: u64 = raw.trim().parse().ok()?;
        // A zero or unparseable value is ignored rather than treated as "no
        // textures": the derivation's floor is what protects a small device,
        // and a typo in an environment variable should not blank the document.
        (mib > 0).then(|| mib * 1024 * 1024)
    })
}

/// The current budget in bytes, derived from the live device profile.
///
/// Call from a component: it reads the profile through context, so it
/// participates in reactivity and the budget follows a probe landing.
#[must_use]
pub fn current() -> u64 {
    let profile = use_device_profile();
    TextureBudget::derive(BudgetInputs {
        available_ram_bytes: profile.available_ram_bytes,
        total_ram_bytes: profile.system_ram_bytes,
        // `Unknown` stays `None` — "not probed yet" must not read as "no GPU",
        // or every session would sit on the budget floor until the first tile
        // paints and the adapter is observed (L9-009).
        gpu_paint_path: match profile.gpu_class {
            GpuClass::Unknown => None,
            other => Some(other.supports_gpu_paint()),
        },
        user_override_bytes: override_bytes(),
    })
    .bytes()
}

/// Physical pixels per CSS pixel on the display this window is on.
///
/// TODO(device-profile-dpr): probe the real value. Blitz owns the factor — it
/// reaches the paint source as `CustomPaintSource::render`'s `scale` argument —
/// but nothing surfaces it to the application, and the tile planner needs it
/// *before* the render callback in order to decide what to mount. Until a probe
/// lands this returns 1.0, which under-states the texture cost on a HiDPI
/// display. The failure direction is benign — under-planning over-mounts, it
/// never evicts, so the cost is a missed saving and never a blank page.
///
/// **The magnitude is not benign, which is why this is Spec 08 R27 and blocks
/// Phase 2 from closing.** Residency is *quadratic* in this factor and only
/// sub-quadratic in zoom (the virtualization window is measured in CSS pixels,
/// so DPI scales both tile dimensions with the mounted count unchanged, while
/// zoom enlarges tiles *and* shrinks the count). DPI is therefore the dominant
/// axis, and a 2x display's true requirement is 4x what this reports — enough
/// that the budget does not bind at all and the pressure policy never fires, on
/// exactly the hardware Phase 2 exists to protect.
///
/// The fix has a precedent in this tree: [`crate::device_probe`] already lifts
/// an equally late-bound value — the GPU adapter class, unknowable until the
/// paint path resumes — from a process-wide record into the reactive
/// `DeviceProfile`, holding `Unknown` distinct from a default so "not probed"
/// never reads as an answer. This takes the same shape, recording from
/// `render`'s `scale` rather than from `resume`, and inherits the same one-frame
/// lag in the same benign direction.
#[must_use]
pub fn device_scale_factor() -> f64 {
    1.0
}

#[cfg(test)]
#[path = "texture_budget_tests.rs"]
mod tests;
