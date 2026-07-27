// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 AppThere Loki contributors

//! Timing controls that benches get **by construction** rather than by
//! remembering (Spec 08 ADR L08-035).
//!
//! # Why this exists
//!
//! L08-022 required warm-up and ordering controls, and `texture_residency` has
//! them. `relayout_edit_position` then compared a *warm* incremental relayout
//! against a *cold-cache* full relayout, so the baseline carried first-touch font
//! loading its counterpart had already paid. It reported reuse saving 80% at 100
//! blocks and 50% at 500. Reuse saves nothing; the whole gap was the control that
//! was written down once and not carried across.
//!
//! Remembering is the wrong remedy. A control you must remember to add is one you
//! will omit exactly when your attention is on the thing being measured — which is
//! what happened, in a program that had already learned the lesson. So warm-up and
//! best-of-N live here, and [`compare`] makes the specific mistake above
//! *structurally impossible*: it warms both sides before timing either.
//!
//! # What these controls can and cannot catch
//!
//! Stated because L08-022 also requires saying what a control's power actually is:
//!
//! - **Warm-up** catches first-touch costs — font loading, allocator growth, page
//!   faults on a fresh arena. This is the one that fired here.
//! - **Best-of-N** catches scheduler noise and background interference. It does not
//!   make a measurement correct, only less noisy; a systematically wrong comparison
//!   stays wrong at every N.
//! - **Neither** catches a fixture that does not exercise the path under test. That
//!   needs an assertion about the path itself — see `relayout_edit_position`'s
//!   check that the incremental path is entered at all.

#![allow(dead_code)] // Each bench uses a different subset.

use std::time::Instant;

/// Timed runs per measurement. Three is enough to drop an obvious outlier without
/// making a sweep tedious; the estimator is the minimum, not the mean, because the
/// quantity of interest is the cost without interference.
pub const RUNS: usize = 3;

/// One measurement: the best of [`RUNS`] timed passes after a warm-up.
#[derive(Clone, Copy, PartialEq, Debug)]
pub struct Timing {
    /// Best observed wall time, in milliseconds.
    pub best_ms: f64,
}

impl Timing {
    /// This timing as a ratio of `other`, e.g. `1.31` = 31% more expensive.
    #[must_use]
    pub fn ratio_to(self, other: Timing) -> f64 {
        if other.best_ms == 0.0 {
            return f64::NAN;
        }
        self.best_ms / other.best_ms
    }
}

/// Times `f` as the best of [`RUNS`] passes, after one untimed warm-up pass.
///
/// Prefer [`compare`] whenever the figure will be read against another figure —
/// this function cannot know what it is being compared with, so it cannot
/// guarantee the two shared a warm state.
pub fn timed(mut f: impl FnMut()) -> Timing {
    f();
    let mut best = f64::MAX;
    for _ in 0..RUNS {
        let t = Instant::now();
        f();
        best = best.min(t.elapsed().as_secs_f64() * 1000.0);
    }
    Timing { best_ms: best }
}

/// Times two alternatives against each other, warming **both** before timing
/// **either**.
///
/// That ordering is the entire point. Warming each side immediately before its own
/// timed runs is not sufficient when the two share mutable state — a font cache, an
/// allocator arena, a memoisation table — because whichever runs first pays the
/// first-touch cost and the other inherits it warm. Running both warm-ups first
/// puts them on the same footing whatever they share.
///
/// Returns `(a, b)` in argument order.
pub fn compare(mut a: impl FnMut(), mut b: impl FnMut()) -> (Timing, Timing) {
    // Both warm-ups precede both measurements — see above.
    a();
    b();
    let mut best_a = f64::MAX;
    let mut best_b = f64::MAX;
    for _ in 0..RUNS {
        let t = Instant::now();
        a();
        best_a = best_a.min(t.elapsed().as_secs_f64() * 1000.0);
        let t = Instant::now();
        b();
        best_b = best_b.min(t.elapsed().as_secs_f64() * 1000.0);
    }
    (Timing { best_ms: best_a }, Timing { best_ms: best_b })
}

/// Times N subjects, warming **all** of them before timing **any** of them.
///
/// The n-ary [`compare`]. Three sequential [`timed`] calls against one shared
/// `FontResources` cannot get this control, and that produced a real false finding:
/// an apparent "cost falls with edit position" that was the first call paying
/// process- and document-level first touch while each later one inherited it warm.
/// The sign reversed when the order reversed.
///
/// The rule — *when several measurements share warm-able state, warm every subject
/// before timing any of them* — was first written down as a note in this module's
/// docs, which is exactly the form L08-035 says does not hold. This function is the
/// enforceable version.
///
/// Subjects are `&mut dyn FnMut()` so heterogeneous closures can share a slice.
/// Results are returned in argument order.
pub fn sweep(subjects: &mut [&mut dyn FnMut()]) -> Vec<Timing> {
    for subject in subjects.iter_mut() {
        subject();
    }
    let mut best = vec![f64::MAX; subjects.len()];
    for _ in 0..RUNS {
        for (i, subject) in subjects.iter_mut().enumerate() {
            let t = Instant::now();
            subject();
            best[i] = best[i].min(t.elapsed().as_secs_f64() * 1000.0);
        }
    }
    best.into_iter().map(|best_ms| Timing { best_ms }).collect()
}

/// How reproducible this harness's own output is for one unchanging subject.
#[derive(Clone, Copy, PartialEq, Debug)]
pub struct Repeatability {
    /// Lowest reported figure across rounds.
    pub best_ms: f64,
    /// Highest reported figure across rounds.
    pub worst_ms: f64,
}

impl Repeatability {
    /// Worst over best — 1.20 means the same subject varied by 20%.
    #[must_use]
    pub fn spread_ratio(self) -> f64 {
        if self.best_ms == 0.0 {
            return f64::NAN;
        }
        self.worst_ms / self.best_ms
    }
}

/// Measures the harness against an unchanging subject: the **noise floor** below
/// which no difference this bench reports means anything.
///
/// # Why a chosen band is not enough
///
/// [`verdict`]'s ±5% is a *chosen* threshold, and nothing established that this
/// harness resolves 5%. If real round-to-round spread on one subject is 20%, then
/// `verdict` will confidently report non-parity on noise — in the same register
/// that produced two retracted attributions. Measure the floor, then read
/// differences against it.
///
/// Note what is being repeated: the whole [`timed`] measurement, not a single pass.
/// The quantity that needs a floor is the figure actually reported, and best-of-N
/// is already more stable than one run.
pub fn repeatability(rounds: usize, mut f: impl FnMut()) -> Repeatability {
    let mut best_ms = f64::MAX;
    let mut worst_ms = 0.0_f64;
    for _ in 0..rounds.max(2) {
        let t = timed(&mut f).best_ms;
        best_ms = best_ms.min(t);
        worst_ms = worst_ms.max(t);
    }
    Repeatability { best_ms, worst_ms }
}

/// Renders a comparison verdict, so benches phrase "faster"/"slower" identically
/// and a reader can scan a column without re-deriving the direction each time.
#[must_use]
pub fn verdict(subject: Timing, baseline: Timing) -> String {
    verdict_within(subject, baseline, 0.05)
}

/// [`verdict`] against a **measured** noise floor rather than the default band.
///
/// Prefer this wherever a [`repeatability`] figure exists: it is the difference
/// between "below a threshold someone picked" and "below what this instrument can
/// resolve today, on this machine".
#[must_use]
pub fn verdict_within(subject: Timing, baseline: Timing, floor_ratio: f64) -> String {
    let r = subject.ratio_to(baseline);
    if !r.is_finite() {
        return "n/a".to_string();
    }
    // Inside this band the two are not distinguishable by this harness, and
    // saying so is more honest than printing a signed percentage that invites a
    // conclusion the measurement cannot support.
    let lo = 1.0 - floor_ratio;
    let hi = 1.0 + floor_ratio;
    if (lo..=hi).contains(&r) {
        return "parity".to_string();
    }
    if r < 1.0 {
        format!("saves {:.0}%", 100.0 * (1.0 - r))
    } else {
        format!("costs {:.0}% more", 100.0 * (r - 1.0))
    }
}

#[cfg(test)]
mod tests {
    use super::{Timing, verdict};

    /// The band matters more than the arithmetic: a 3% difference must not read as
    /// a finding, because this harness cannot resolve it.
    #[test]
    fn small_differences_report_parity_rather_than_a_percentage() {
        let base = Timing { best_ms: 100.0 };
        assert_eq!(verdict(Timing { best_ms: 103.0 }, base), "parity");
        assert_eq!(verdict(Timing { best_ms: 97.0 }, base), "parity");
        assert_eq!(verdict(Timing { best_ms: 131.0 }, base), "costs 31% more");
        assert_eq!(verdict(Timing { best_ms: 50.0 }, base), "saves 50%");
    }
}
