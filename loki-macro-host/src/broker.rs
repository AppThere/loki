// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 AppThere Loki contributors

//! The capability broker — the [`loki_basic::Host`] implementation that gates
//! every macro effect (macro spec §4.3, §5.1).
//!
//! The interpreter has no authority of its own; the broker *is* the authority.
//! For a single macro run it holds:
//!
//! - the **run context** ([`RunContext`]) — interactive runs may prompt; a
//!   spreadsheet UDF runs with zero capabilities and can never prompt (§6.3);
//! - the **resolved allow-set** — capabilities already permitted for this run,
//!   folded from the document's persisted always-grants plus this session's
//!   grants before the run starts;
//! - **once-grants** accumulated during the run (a prompt answered "Allow
//!   once");
//! - **fuel** and a shared **cancel** flag (§8) — the interpreter's only
//!   resource channel.
//!
//! Phase 4 builds and tests the decision surface; the request-routing that
//! turns a [`CapabilityDecision::Prompt`] into a live UI prompt and an effect is
//! Phase 5. Nothing here executes an effect.

use std::collections::BTreeSet;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};

use loki_basic::{FuelVerdict, Host};

use crate::capability::{Capability, CapabilityDecision, GrantScope, RunContext};
use crate::trust::TrustRecord;

/// The capabilities permitted for a run, resolved before it starts.
///
/// Built from a document's persisted always-for-document grants
/// ([`Self::from_record`]) plus any session grants the caller folds in with
/// [`Self::allow`]. Baseline capabilities (`DocRead`) are *not* stored here —
/// they are always granted by [`CapabilityBroker::evaluate`] regardless.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct GrantSet {
    allowed: BTreeSet<Capability>,
}

impl GrantSet {
    /// An empty grant set.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// The always-for-document grants persisted on `record` (or empty when the
    /// document has no trust record). Refused capabilities are never folded in,
    /// even if a stale record somehow lists one.
    #[must_use]
    pub fn from_record(record: Option<&TrustRecord>) -> Self {
        let mut set = Self::new();
        if let Some(rec) = record {
            for grant in &rec.capability_grants {
                if grant.scope.is_allow() && !grant.capability.is_refused_in_v1() {
                    set.allowed.insert(grant.capability);
                }
            }
        }
        set
    }

    /// Adds an allow for `cap` (used to fold session grants into the run). A
    /// refused capability is ignored — refusal can never be granted around.
    pub fn allow(&mut self, cap: Capability) {
        if !cap.is_refused_in_v1() {
            self.allowed.insert(cap);
        }
    }

    /// Whether `cap` is in the allow-set.
    #[must_use]
    pub fn contains(&self, cap: Capability) -> bool {
        self.allowed.contains(&cap)
    }
}

/// Gates macro effects for one run, and meters its fuel.
#[derive(Debug)]
pub struct CapabilityBroker {
    context: RunContext,
    resolved: GrantSet,
    once: BTreeSet<Capability>,
    remaining_fuel: u64,
    cancel: Arc<AtomicBool>,
}

impl CapabilityBroker {
    /// Creates a broker for an interactive run with `resolved` grants and a
    /// fuel budget, sharing `cancel` with the UI's Stop control.
    #[must_use]
    pub fn new(resolved: GrantSet, fuel: u64, cancel: Arc<AtomicBool>) -> Self {
        Self {
            context: RunContext::Interactive,
            resolved,
            once: BTreeSet::new(),
            remaining_fuel: fuel,
            cancel,
        }
    }

    /// Creates a compute-only broker for a spreadsheet UDF: zero capabilities,
    /// no prompts, a fixed fuel budget, and no cancel affordance (recalc is
    /// unattended — spec §6.3, §8).
    #[must_use]
    pub fn for_udf(fuel: u64) -> Self {
        Self {
            context: RunContext::Udf,
            resolved: GrantSet::new(),
            once: BTreeSet::new(),
            remaining_fuel: fuel,
            cancel: Arc::new(AtomicBool::new(false)),
        }
    }

    /// The run context.
    #[must_use]
    pub fn context(&self) -> RunContext {
        self.context
    }

    /// Decides how a requested capability is handled (spec §5.1). This is the
    /// heart of the broker and is deliberately a pure function of the current
    /// grant state, so the capability matrix is exhaustively testable.
    #[must_use]
    pub fn evaluate(&self, cap: Capability) -> CapabilityDecision {
        // 1. Permanent refusals win over everything (spec §7). No context,
        //    grant, or prompt can reach a refused capability.
        if cap.is_refused_in_v1() {
            return CapabilityDecision::Refused;
        }
        // 2. UDFs have zero authority and cannot prompt (spec §6.3): every
        //    effect is a trappable denial surfaced as `#MACRO!`.
        if self.context == RunContext::Udf {
            return CapabilityDecision::Denied;
        }
        // 3. Baseline capability, granted for any enabled document (spec §5.2).
        if cap.is_baseline() {
            return CapabilityDecision::Granted;
        }
        // 4. An existing allow (persisted, session, or once) short-circuits the
        //    prompt.
        if self.resolved.contains(cap) || self.once.contains(&cap) {
            return CapabilityDecision::Granted;
        }
        // 5. Otherwise the user must decide at first use.
        CapabilityDecision::Prompt
    }

    /// Applies the user's answer to a first-use prompt for `cap` (spec §5.4).
    ///
    /// Returns whether the effect may now proceed. `AllowOnce`/`AllowSession`
    /// are folded into this run so the same capability is not re-prompted;
    /// persisting an `AlwaysForDocument` grant to the trust store, and
    /// remembering `AllowSession` beyond this run, are the caller's job
    /// (`MacroService`). `Deny` records nothing — a later request re-prompts.
    /// A refused capability can never be granted, whatever the answer.
    pub fn apply_prompt(&mut self, cap: Capability, scope: GrantScope) -> bool {
        if cap.is_refused_in_v1() || !scope.is_allow() {
            return false;
        }
        // Any allow at least covers the remainder of this run.
        self.once.insert(cap);
        if matches!(
            scope,
            GrantScope::AllowSession | GrantScope::AlwaysForDocument
        ) {
            self.resolved.allow(cap);
        }
        true
    }

    /// Fuel still available (spec §8).
    #[must_use]
    pub fn remaining_fuel(&self) -> u64 {
        self.remaining_fuel
    }

    /// Requests cancellation from another thread (the UI Stop control).
    pub fn request_cancel(&self) {
        self.cancel.store(true, Ordering::SeqCst);
    }

    /// A handle to the shared cancel flag, so the UI thread can trip it.
    #[must_use]
    pub fn cancel_flag(&self) -> Arc<AtomicBool> {
        Arc::clone(&self.cancel)
    }
}

impl Host for CapabilityBroker {
    fn consume_fuel(&mut self, units: u64) -> FuelVerdict {
        // Cancellation is checked first so a runaway macro stops promptly even
        // while it still has fuel (spec §8: Stop is always available).
        if self.cancel.load(Ordering::SeqCst) {
            return FuelVerdict::Cancelled;
        }
        match self.remaining_fuel.checked_sub(units) {
            Some(rest) => {
                self.remaining_fuel = rest;
                FuelVerdict::Continue
            }
            None => {
                self.remaining_fuel = 0;
                FuelVerdict::Exhausted
            }
        }
    }
}

#[cfg(test)]
#[path = "broker_tests.rs"]
mod tests;
