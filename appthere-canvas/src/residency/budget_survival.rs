// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 AppThere Loki contributors

//! The **survival ceiling** — the threshold at which the renderer stops
//! protecting resolution and starts protecting the process (Spec 08 ADR L08-026,
//! L08-027).
//!
//! Split from `budget.rs` to keep that file under the 300-line ceiling. The
//! cluster is cohesive on its own terms: these are the only constants that
//! govern *degrading what the user is looking at*, as opposed to the byte target
//! in the parent, which governs what the user cannot see.

/// Share of *available* RAM at which the renderer stops protecting resolution
/// and starts protecting the process: one eighth.
///
/// This is **not** the budget. It is the line past which continuing to honour
/// full-resolution visible pages risks the OS killing us, and a soft page beats
/// a dead application. See [`super::TextureBudget::hard_ceiling_bytes`] for why the two
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

/// Absolute cap on the survival ceiling, regardless of how much RAM the machine
/// has (1 GiB).
///
/// [`SURVIVAL_AVAILABLE_RAM_DIVISOR`] alone is purely proportional, and
/// proportional has no opinion about absurdity: a 64 GiB workstation reporting
/// ~50 GiB available would derive a **6.4 GiB** texture ceiling for a word
/// processor, and nothing else in the policy would object. The budget has had a
/// floor and a ceiling clamp since T2.1 for exactly this reason; the survival
/// line needs the same treatment.
///
/// # Why more than this buys nothing a reader can perceive
///
/// The cap is not "enough pixels for the screen" — it is far more than that, and
/// deliberately so. It is the point past which the extra bytes are overwhelmingly
/// **page area that is not on screen**. Tiles are whole pages, so a page counts as
/// visible when any part of it overlaps the viewport: at 400% zoom a US Letter
/// page is ~12,700 device pixels tall on a 3x display against a viewport of
/// ~1,800, so roughly 85% of a "visible" page's texture is off-screen at any
/// moment. Raising the ceiling past 1 GiB spends memory almost entirely on page
/// area the reader is not looking at, which is precisely what the *target* exists
/// to avoid spending on.
///
/// TODO(subpage-tiling): the honest fix for that regime is to rasterise the
/// visible band of a page rather than the whole page, which would make visible
/// residency proportional to viewport area — bounded by the display and
/// independent of zoom — instead of to page area. That is a change to the
/// whole-page `PageTile` abstraction rather than to this policy, so it is future
/// work; this cap is the bound until it lands.
pub const SURVIVAL_CAP_BYTES: u64 = 1024 * 1024 * 1024;

/// Share of *total* RAM used for the survival ceiling when available is missing:
/// one sixteenth. Agrees with [`SURVIVAL_AVAILABLE_RAM_DIVISOR`] at the design
/// floor by the same construction as [`super::TOTAL_RAM_DIVISOR`].
pub const SURVIVAL_TOTAL_RAM_DIVISOR: u64 = 16;

/// Applies [`SURVIVAL_CAP_BYTES`] to a proportionally-derived survival ceiling,
/// while keeping the `ceiling >= target` invariant.
///
/// The target wins over the cap when they conflict, because a target above 1 GiB
/// can only come from a person setting one explicitly, and the cap exists to stop
/// an *automatic* derivation running away — not to overrule someone who knows
/// their machine (the same division [`super::BUDGET_CEILING_BYTES`] already draws).
pub(crate) fn survival_ceiling(derived: u64, target: u64) -> u64 {
    derived.min(SURVIVAL_CAP_BYTES).max(target)
}
