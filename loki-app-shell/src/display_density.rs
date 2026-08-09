// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 AppThere Loki contributors

//! Where a display's pixel density comes from, and what makes a reported one
//! believable (Spec 08 T5.5).
//!
//! # The platform lies, and this environment proves it
//!
//! T5.5 says "Actual Size from platform physical-size query … falling back to
//! per-display user calibration". It is easy to read the fallback as the
//! *unavailable* case — the platform did not answer — and to trust the answer
//! whenever there is one. That is wrong, and the evidence is in this repository's
//! own test logs: the first screen sitting produced
//!
//! ```text
//! WARN winit: XRandR reported that the display's 0mm in size, which is certifiably insane
//! ```
//!
//! A display reporting 0 mm yields an infinite ppi. Monitors with no EDID, KVM
//! switches, projectors and virtual displays all report nonsense of this kind,
//! and a *wrong* density is worse than none: Actual Size then prints a page that
//! is confidently the wrong physical size, and the reader has no way to tell a
//! wrong ruler from a right one. So every reported density passes
//! [`plausible_ppi`] before it is believed, and an implausible one takes the same
//! path as no answer at all.
//!
//! # CSS pixels per inch, computed once, here
//!
//! A platform query reports **device** pixels and millimetres.
//! `actual_size_zoom_percent` needs **CSS** pixels per inch. They differ by the
//! device scale factor, which on the machine named in T5.5's own acceptance
//! criterion is 2 — so confusing them is a 100% error on exactly the display the
//! criterion is measured against. [`css_ppi_from_physical`] is the one place the
//! division happens; `PhysicalDisplay::css_px_per_inch` is named for what it
//! holds so that no caller has to remember which it received.

/// Millimetres per inch.
const MM_PER_INCH: f32 = 25.4;

/// Where a density came from.
///
/// Kept alongside the number because the two answer different questions: the
/// value drives the zoom, and the source decides whether the app may offer
/// calibration. A calibrated display must not be re-prompted, and a
/// platform-reported one must still be correctable by a reader whose ruler
/// disagrees.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum DensitySource {
    /// Queried from the windowing system.
    Platform,
    /// Measured by the reader against a real ruler.
    Calibrated,
}

/// A believable display density, and where it came from.
#[derive(Clone, Copy, PartialEq, Debug)]
pub struct DisplayDensity {
    /// CSS pixels per inch — the quantity Actual Size consumes.
    pub css_px_per_inch: f32,
    pub source: DensitySource,
}

/// Whether a **device** pixels-per-inch figure is worth believing.
///
/// The bounds are deliberately wide: this rejects nonsense, not unusual
/// hardware. A 55-inch 4K television sits near 80 ppi and a recent phone near
/// 500, so anything inside 40..=800 is somebody's real screen and must be
/// accepted. What it rules out is the failure *modes* — zero or negative
/// dimensions (an absent EDID), non-finite values (a division by a zero
/// dimension), and figures far outside any panel ever shipped.
///
/// A narrower range would be an instrument that rejects the phenomenon along
/// with the noise.
#[must_use]
pub fn plausible_ppi(ppi: f32) -> bool {
    ppi.is_finite() && (40.0..=800.0).contains(&ppi)
}

/// Device pixels per inch from a monitor's pixel size and physical size in
/// millimetres, or `None` when either is unusable.
///
/// Takes the **width** axis. A monitor whose two axes disagree is reporting at
/// least one of them wrongly, and averaging would turn one bad number into two
/// slightly bad ones; the width is the axis a page is fitted against, so it is
/// the one to be right about.
#[must_use]
pub fn device_ppi_from_mm(width_px: u32, width_mm: u32) -> Option<f32> {
    if width_px == 0 || width_mm == 0 {
        return None;
    }
    let ppi = width_px as f32 / (width_mm as f32 / MM_PER_INCH);
    plausible_ppi(ppi).then_some(ppi)
}

/// CSS pixels per inch from a device figure and the display's scale factor.
///
/// `None` for a scale factor that is not a positive finite number — the same
/// rejection the density itself gets, because a bad divisor produces a bad
/// quotient and the quotient is what reaches the zoom.
#[must_use]
pub fn css_ppi_from_physical(device_ppi: f32, device_scale_factor: f64) -> Option<f32> {
    if !device_scale_factor.is_finite() || device_scale_factor <= 0.0 {
        return None;
    }
    let css = device_ppi / device_scale_factor as f32;
    css.is_finite().then_some(css)
}

/// The density a reader measured: they were shown a line the app believes is
/// `nominal_mm` long, and reported its true length as `measured_mm`.
///
/// # Why a ratio rather than "type your screen's ppi"
///
/// Asking for a ppi asks the reader to know a number about their hardware that
/// most people cannot look up and that many vendors state wrongly. Asking them
/// to hold a ruler against a drawn line asks for the measurement the number is
/// *for*, which is a question anyone can answer correctly with a ruler and a
/// bank card.
///
/// `None` when the measurement cannot be believed — a non-positive length, or a
/// correction so large that the reader has almost certainly answered in the
/// wrong units. The wrong-units case is the common one (centimetres for
/// millimetres is a factor of ten), and accepting it would silently calibrate the
/// display an order of magnitude out.
#[must_use]
pub fn calibrated_css_ppi(
    assumed_css_ppi: f32,
    nominal_mm: f32,
    measured_mm: f32,
) -> Option<DisplayDensity> {
    if !(nominal_mm.is_finite() && measured_mm.is_finite())
        || nominal_mm <= 0.0
        || measured_mm <= 0.0
    {
        return None;
    }
    let ratio = nominal_mm / measured_mm;
    // A real display is within a factor of ~2 of the CSS assumption; beyond that
    // the reader has mis-measured or mis-read the units.
    if !(0.5..=2.0).contains(&ratio) {
        return None;
    }
    let css = assumed_css_ppi * ratio;
    plausible_ppi(css).then_some(DisplayDensity {
        css_px_per_inch: css,
        source: DensitySource::Calibrated,
    })
}

#[cfg(test)]
#[path = "display_density_tests.rs"]
mod tests;
