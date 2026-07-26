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

/// The current budget, derived from the live device profile.
///
/// Call from a component: it reads the profile through context, so it
/// participates in reactivity and the budget follows a probe landing.
///
/// Returns the whole [`TextureBudget`] rather than its byte target. Handing the
/// renderer a bare `u64` was a real defect until r18: it rebuilt the budget with
/// `TextureBudget::with_baseline_ceiling`, which derives a survival ceiling from the *baseline*
/// rather than from this device, so every machine got 512 MiB — including a phone
/// that had correctly derived 256 MiB. Deriving a value and then not delivering it
/// is indistinguishable from never deriving it (L08-028).
#[must_use]
pub fn current() -> TextureBudget {
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
}

/// Physical pixels per CSS pixel on the display this window is on.
///
/// Reads the value the paint path last rendered at (Spec 08 R27), falling back to
/// 1.0 before the first page tile has painted. `DeviceProfile` is the source of
/// truth so the value is reactive: when the sensor observes a change — first
/// paint, or a window dragged between displays — every plan downstream of this
/// recomputes.
///
/// **Why the fallback is 1.0 and not something larger.** Under-stating the factor
/// over-mounts for one frame; over-stating it would reduce rasterisation scale on
/// a display that did not need it, which is visible. Between a frame of extra
/// residency and a frame of soft text, residency is the right thing to spend —
/// the same asymmetry that makes the whole target-versus-ceiling split work
/// (ADR L08-026).
#[must_use]
pub fn device_scale_factor() -> f64 {
    use_device_profile()
        .device_scale_factor
        .filter(|v| v.is_finite() && *v > 0.0)
        .unwrap_or(1.0)
}

#[cfg(test)]
#[path = "texture_budget_tests.rs"]
mod tests;
