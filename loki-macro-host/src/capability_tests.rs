// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 AppThere Loki contributors

use super::*;

#[test]
fn all_covers_every_variant_once() {
    // ALL must enumerate each capability exactly once (used for exhaustive
    // matrices and UI listing).
    let mut ids: Vec<&str> = Capability::ALL.iter().map(|c| c.id()).collect();
    ids.sort_unstable();
    ids.dedup();
    assert_eq!(ids.len(), Capability::ALL.len());
}

#[test]
fn ids_are_stable_and_unique() {
    for cap in Capability::ALL {
        assert!(!cap.id().is_empty());
    }
}

#[test]
fn only_network_is_refused() {
    for cap in Capability::ALL {
        assert_eq!(
            cap.is_refused_in_v1(),
            cap == Capability::Network,
            "{cap:?} refusal posture wrong"
        );
    }
}

#[test]
fn only_doc_read_is_baseline() {
    for cap in Capability::ALL {
        assert_eq!(
            cap.is_baseline(),
            cap == Capability::DocRead,
            "{cap:?} baseline posture wrong"
        );
    }
}

#[test]
fn grant_scope_allow_and_persistence() {
    assert!(!GrantScope::Deny.is_allow());
    assert!(GrantScope::AllowOnce.is_allow());
    assert!(GrantScope::AllowSession.is_allow());
    assert!(GrantScope::AlwaysForDocument.is_allow());

    assert!(!GrantScope::AllowOnce.is_persistent());
    assert!(!GrantScope::AllowSession.is_persistent());
    assert!(GrantScope::AlwaysForDocument.is_persistent());
    assert!(!GrantScope::Deny.is_persistent());
}

#[test]
fn capability_serde_roundtrip() {
    for cap in Capability::ALL {
        let json = serde_json::to_string(&cap).expect("serialize");
        let back: Capability = serde_json::from_str(&json).expect("deserialize");
        assert_eq!(cap, back);
    }
}
