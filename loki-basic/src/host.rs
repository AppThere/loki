// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 AppThere Loki contributors

//! The [`Host`] seam — the interpreter's *only* channel to the outside world.
//!
//! The interpreter has no ambient authority (macro spec §4.3): it can evaluate
//! expressions and mutate its own heap, but anything observable — resource
//! accounting today; dialogs, document reads/writes, and the object model in
//! later phases — is routed through a `Host`. A `loki-basic` embedded with the
//! [`NullHost`] is a pure calculator with unlimited fuel; a real embedding
//! supplies a metering, capability-brokering host (`loki-macro-host`).
//!
//! Phase 2 defines only the fuel channel; the capability-request surface
//! (`HostRequest`/`HostReply`, object-model roots) is added in later phases
//! without changing this trait's fuel contract.

/// The host's verdict when the interpreter asks to spend fuel (spec §8).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FuelVerdict {
    /// Fuel remained; keep executing.
    Continue,
    /// The budget is exhausted — the interpreter must stop with an untrappable
    /// [`crate::RuntimeError::fuel_exhausted`].
    Exhausted,
    /// The host cancelled execution (user pressed Stop). The interpreter stops
    /// with an untrappable [`crate::RuntimeError::cancelled`].
    Cancelled,
}

/// The interpreter's sole interface to its embedding.
///
/// Implementors decide how much work a macro may do (fuel) and, in later
/// phases, which effects it may perform. The interpreter calls
/// [`Host::consume_fuel`] as it walks the tree; returning anything other than
/// [`FuelVerdict::Continue`] halts the run.
pub trait Host {
    /// Accounts `units` of work against the budget and reports whether
    /// execution may continue. Called frequently (per statement / loop
    /// iteration / expression step), so implementations must be cheap.
    fn consume_fuel(&mut self, units: u64) -> FuelVerdict;
}

/// A host that grants unlimited fuel and no capabilities — the "pure
/// calculator" embedding used for expression evaluation and tests.
#[derive(Debug, Default, Clone, Copy)]
pub struct NullHost;

impl Host for NullHost {
    fn consume_fuel(&mut self, _units: u64) -> FuelVerdict {
        FuelVerdict::Continue
    }
}

/// A simple fuel-metering host: grants a fixed budget, then reports
/// [`FuelVerdict::Exhausted`]. Useful for tests and simple embeddings; the real
/// app host also folds in cancellation and a wall-clock watchdog.
#[derive(Debug, Clone, Copy)]
pub struct FuelBudget {
    remaining: u64,
}

impl FuelBudget {
    /// Creates a budget of `total` fuel units.
    #[must_use]
    pub fn new(total: u64) -> Self {
        Self { remaining: total }
    }

    /// Fuel units still available.
    #[must_use]
    pub fn remaining(&self) -> u64 {
        self.remaining
    }
}

impl Host for FuelBudget {
    fn consume_fuel(&mut self, units: u64) -> FuelVerdict {
        if let Some(rest) = self.remaining.checked_sub(units) {
            self.remaining = rest;
            FuelVerdict::Continue
        } else {
            self.remaining = 0;
            FuelVerdict::Exhausted
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn null_host_never_exhausts() {
        let mut h = NullHost;
        assert_eq!(h.consume_fuel(u64::MAX), FuelVerdict::Continue);
    }

    #[test]
    fn budget_exhausts_when_spent() {
        let mut h = FuelBudget::new(10);
        assert_eq!(h.consume_fuel(4), FuelVerdict::Continue);
        assert_eq!(h.consume_fuel(6), FuelVerdict::Continue);
        assert_eq!(h.remaining(), 0);
        assert_eq!(h.consume_fuel(1), FuelVerdict::Exhausted);
    }
}
