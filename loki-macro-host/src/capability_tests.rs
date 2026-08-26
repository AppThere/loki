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

/// Golden list of the capability ids, in `Capability::ALL` order.
///
/// The previous test asserts only non-emptiness, so nothing pinned these
/// strings. Each id is an **external contract** in two places, neither of which
/// a symmetric test can see:
///
/// - the i18n lookup suffix — `macros-cap-<id>-name` / `-title` /
///   `-consequence` in `loki-text`'s consent prompt and Document Security
///   panel. A renamed id resolves to no Fluent message, so the user is asked to
///   grant a capability whose name and consequence render as raw keys.
/// - the macro-author-visible error text — `feature_refused(cap.id())` and
///   `"Permission denied: {id}"` in `exec`.
///
/// Note that the id is **not** what the trust store persists: `PersistedGrant`
/// serializes the `Capability` enum itself, so the on-disk spelling is the
/// serde variant name — pinned separately by
/// [`serde_variant_names_match_the_persisted_golden_list`].
///
/// Appending a new capability is additive and only extends this list; changing
/// an existing entry breaks the contracts above.
#[test]
fn ids_match_the_golden_list() {
    let ids: Vec<&str> = Capability::ALL.iter().map(|c| c.id()).collect();
    assert_eq!(
        ids,
        [
            "doc-read",
            "doc-write",
            "ui-dialog",
            "clipboard-read",
            "clipboard-write",
            "file-read",
            "file-write",
            "print",
            "network",
        ],
        "capability ids are an i18n and error-text contract — see this test's doc comment"
    );
}

/// The **on-disk** capability vocabulary: `TrustRecord::capability_grants` holds
/// `PersistedGrant { capability, scope }` and the whole store is `serde_json`,
/// so a `Capability`'s variant name is literally what sits in the user's trust
/// file and is read back on the next launch.
///
/// Renaming a variant therefore orphans every stored grant under the old
/// spelling: an `AlwaysForDocument` allow silently reverts to a prompt, and a
/// persisted refusal silently stops applying. `capability_serde_roundtrip` is
/// symmetric — it serializes and deserializes with the same build — so it
/// cannot see that. Pin the wire names as literals instead.
///
/// Changing any name below is a **breaking change to the trust store**,
/// requiring a migration that rewrites existing records — not an updated
/// expectation here.
#[test]
fn serde_variant_names_match_the_persisted_golden_list() {
    let json = serde_json::to_string(&Capability::ALL.to_vec()).expect("serialize");
    assert_eq!(
        json,
        r#"["DocRead","DocWrite","UiDialog","ClipboardRead","ClipboardWrite","FileRead","FileWrite","Print","Network"]"#
    );
    // Decode direction, from a literal an older build would have written.
    let back: Capability = serde_json::from_str(r#""ClipboardWrite""#).expect("deserialize");
    assert_eq!(back, Capability::ClipboardWrite);
}

/// `GrantScope` is persisted alongside the capability in the same record.
#[test]
fn grant_scope_variant_names_match_the_persisted_golden_list() {
    let scopes = [
        GrantScope::Deny,
        GrantScope::AllowOnce,
        GrantScope::AllowSession,
        GrantScope::AlwaysForDocument,
    ];
    let json = serde_json::to_string(&scopes.to_vec()).expect("serialize");
    assert_eq!(
        json,
        r#"["Deny","AllowOnce","AllowSession","AlwaysForDocument"]"#
    );
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
