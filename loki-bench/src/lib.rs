// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 AppThere Loki contributors

#![forbid(unsafe_code)]

//! Shared benchmarking & continuous-memory-tracking harness for the AppThere
//! Loki suite (Spec 06). Lives at the monorepo root alongside the Spec 01/02
//! shared infrastructure so Text, Presentation, and Spreadsheet can all bench.
//!
//! # The two axes (Spec 06 §5)
//!
//! Every bench is sorted onto exactly one [`Axis`], because *where it can run*
//! and *how trustworthy its number is* differ sharply:
//!
//! - [`Axis::Portable`] — heap allocation bytes/counts (dhat), model/layout/IO/
//!   style op counts, `vello_cpu` render cost. Hardware-independent, so it runs
//!   headless in the agent environment with **no GPU**, and its numbers are
//!   diffable across machines and over time. This is what makes memory
//!   *continuously trackable* (decision D1).
//! - [`Axis::DeviceBound`] — GPU frame-time, wall-clock latency, real peak RSS.
//!   Varies by CPU/GPU/driver, so it runs **only on real hardware** and is a
//!   local reference point, never a tracked cross-machine signal.
//!
//! [`Metric`] pins each measurement to its axis, encoding the §5 table directly:
//! allocation bytes/counts are Portable; RSS and frame-time are DeviceBound.
//!
//! # Memory measurement
//!
//! [`measure`] runs a workload under a dhat heap-profiling session and returns
//! the portable [`AllocStats`] it produced. dhat only records when its global
//! allocator is installed in the running binary, so a bench binary opts in with
//! [`dhat_global_allocator!`] at its top. See `benches/portable_alloc.rs`.
//!
//! # Status (Spec 06 M1)
//!
//! This is the **harness skeleton + two-axis split**: the axis/metric model, the
//! dhat allocation-measurement path, and Criterion wired via the `benches/`
//! targets. The per-target portable benches (M2), the committed baseline + diff
//! (M3), leak detection (M4), and the device benches + budgets (M5) build on it.

mod axis;
mod baseline;
mod budget;
mod leak;
mod memory;
mod parity;
mod rss;

pub use axis::{Axis, Metric};
pub use baseline::{
    Baseline, BaselineError, Delta, DeltaStatus, Tolerance, any_regressed, diff, render_report,
};
pub use budget::{BudgetStatus, Budgets, check, headroom_frac};
pub use leak::{LeakVerdict, ResidualStats, classify_leak, residual_after};
pub use memory::{AllocStats, measure};
pub use parity::{
    ParityStatus, confirmed_version_from_marker, parity_status, render_marker,
    vello_version_from_lock,
};
pub use rss::{current_rss_bytes, parse_status_kib, peak_rss_bytes};

/// Re-exported so [`dhat_global_allocator!`] can name `dhat` from any bench
/// binary without that binary declaring a direct `dhat` dependency.
pub use dhat;

/// Installs dhat's global allocator in the current binary so [`measure`] records
/// real allocations. Place it once at the top of a bench/binary crate root:
///
/// ```ignore
/// loki_bench::dhat_global_allocator!();
///
/// fn main() {
///     let stats = loki_bench::measure(|| { /* portable workload */ });
///     eprintln!("{} bytes / {} allocs", stats.total_bytes, stats.total_blocks);
/// }
/// ```
///
/// Without it dhat records nothing and [`measure`] returns zeroed stats — the
/// harness still runs, it just has no memory signal, so opting in is required.
#[macro_export]
macro_rules! dhat_global_allocator {
    () => {
        #[global_allocator]
        static LOKI_BENCH_DHAT_ALLOC: $crate::dhat::Alloc = $crate::dhat::Alloc;
    };
}
