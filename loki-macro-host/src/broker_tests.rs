// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 AppThere Loki contributors

use std::sync::Arc;
use std::sync::atomic::AtomicBool;

use loki_basic::{FuelVerdict, Host};

use super::*;
use crate::capability::{Capability, CapabilityDecision, GrantScope};
use crate::trust::{TrustDecision, TrustRecord};

fn broker(resolved: GrantSet) -> CapabilityBroker {
    CapabilityBroker::new(resolved, 1_000, Arc::new(AtomicBool::new(false)))
}

// ── Default posture (spec §5.2) ──────────────────────────────────────────────

#[test]
fn doc_read_is_granted_without_a_grant() {
    let b = broker(GrantSet::new());
    assert_eq!(b.evaluate(Capability::DocRead), CapabilityDecision::Granted);
}

#[test]
fn sensitive_capabilities_prompt_by_default() {
    let b = broker(GrantSet::new());
    for cap in [
        Capability::DocWrite,
        Capability::UiDialog,
        Capability::ClipboardRead,
        Capability::ClipboardWrite,
        Capability::FileRead,
        Capability::FileWrite,
        Capability::Print,
    ] {
        assert_eq!(b.evaluate(cap), CapabilityDecision::Prompt, "{cap:?}");
    }
}

#[test]
fn network_is_refused_unconditionally() {
    // Even with an (illegitimate) always-grant folded in, refusal wins.
    let mut g = GrantSet::new();
    g.allow(Capability::Network); // ignored — refused caps can't be granted
    let b = broker(g);
    assert_eq!(b.evaluate(Capability::Network), CapabilityDecision::Refused);
}

// ── Grant scopes are honoured (spec §5.4) ────────────────────────────────────

#[test]
fn a_resolved_allow_grants_without_prompting() {
    let mut g = GrantSet::new();
    g.allow(Capability::DocWrite);
    let b = broker(g);
    assert_eq!(
        b.evaluate(Capability::DocWrite),
        CapabilityDecision::Granted
    );
}

#[test]
fn allow_once_covers_the_rest_of_the_run_only() {
    let mut b = broker(GrantSet::new());
    assert_eq!(b.evaluate(Capability::Print), CapabilityDecision::Prompt);
    assert!(b.apply_prompt(Capability::Print, GrantScope::AllowOnce));
    assert_eq!(b.evaluate(Capability::Print), CapabilityDecision::Granted);
}

#[test]
fn deny_does_not_record_and_re_prompts() {
    let mut b = broker(GrantSet::new());
    assert!(!b.apply_prompt(Capability::UiDialog, GrantScope::Deny));
    assert_eq!(b.evaluate(Capability::UiDialog), CapabilityDecision::Prompt);
}

#[test]
fn refused_capability_cannot_be_granted_by_any_answer() {
    let mut b = broker(GrantSet::new());
    for scope in [
        GrantScope::AllowOnce,
        GrantScope::AllowSession,
        GrantScope::AlwaysForDocument,
    ] {
        assert!(!b.apply_prompt(Capability::Network, scope));
        assert_eq!(b.evaluate(Capability::Network), CapabilityDecision::Refused);
    }
}

// ── Grants resolve from the trust record ─────────────────────────────────────

#[test]
fn from_record_folds_in_persisted_grants() {
    let mut rec = TrustRecord::new([9u8; 32], TrustDecision::Trusted);
    rec.set_grant(Capability::DocWrite, GrantScope::AlwaysForDocument);
    let g = GrantSet::from_record(Some(&rec));
    assert!(g.contains(Capability::DocWrite));
    assert!(!g.contains(Capability::Print));
}

#[test]
fn revocation_is_immediate() {
    // Simulate the panel revoking DocWrite: a fresh broker built from the
    // updated record no longer grants it.
    let mut rec = TrustRecord::new([1u8; 32], TrustDecision::Trusted);
    rec.set_grant(Capability::DocWrite, GrantScope::AlwaysForDocument);
    assert!(
        broker(GrantSet::from_record(Some(&rec))).evaluate(Capability::DocWrite)
            == CapabilityDecision::Granted
    );

    rec.revoke(Capability::DocWrite);
    assert_eq!(
        broker(GrantSet::from_record(Some(&rec))).evaluate(Capability::DocWrite),
        CapabilityDecision::Prompt
    );
}

// ── UDF context: compute-only (spec §6.3) ────────────────────────────────────

#[test]
fn udf_denies_every_effect_and_never_prompts() {
    let b = CapabilityBroker::for_udf(500);
    for cap in Capability::ALL {
        let expected = if cap.is_refused_in_v1() {
            CapabilityDecision::Refused
        } else {
            CapabilityDecision::Denied
        };
        assert_eq!(b.evaluate(cap), expected, "{cap:?}");
    }
}

#[test]
fn udf_cannot_be_granted_a_capability() {
    let mut b = CapabilityBroker::for_udf(500);
    // Even DocRead is denied for a UDF (spec §6.3: not even DocRead).
    assert_eq!(b.evaluate(Capability::DocRead), CapabilityDecision::Denied);
    // An accidental prompt-apply still leaves the UDF unable to act.
    let _ = b.apply_prompt(Capability::DocWrite, GrantScope::AllowSession);
    assert_eq!(b.evaluate(Capability::DocWrite), CapabilityDecision::Denied);
}

// ── Fuel + cancel (spec §8) ──────────────────────────────────────────────────

#[test]
fn fuel_exhausts() {
    let mut b = CapabilityBroker::new(GrantSet::new(), 10, Arc::new(AtomicBool::new(false)));
    assert_eq!(b.consume_fuel(6), FuelVerdict::Continue);
    assert_eq!(b.consume_fuel(4), FuelVerdict::Continue);
    assert_eq!(b.consume_fuel(1), FuelVerdict::Exhausted);
}

#[test]
fn cancel_flag_stops_before_fuel_runs_out() {
    let cancel = Arc::new(AtomicBool::new(false));
    let mut b = CapabilityBroker::new(GrantSet::new(), 1_000, Arc::clone(&cancel));
    assert_eq!(b.consume_fuel(1), FuelVerdict::Continue);
    cancel.store(true, std::sync::atomic::Ordering::SeqCst);
    assert_eq!(b.consume_fuel(1), FuelVerdict::Cancelled);
}
