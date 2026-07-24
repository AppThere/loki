// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 AppThere Loki contributors

use loki_doc_model::io::macros::{MacroPayload, MacroPayloadKind, PreservedPart};

use crate::capability::Capability;
use crate::service::MacroService;
use crate::trust::TrustDecision;

fn payload(tag: &[u8]) -> MacroPayload {
    MacroPayload::new(
        MacroPayloadKind::OoxmlVba,
        vec![PreservedPart::new(
            "/word/vbaProject.bin",
            None,
            tag.to_vec(),
        )],
    )
}

#[test]
fn default_decision_is_disabled() {
    let svc = MacroService::in_memory();
    let p = payload(b"a");
    assert_eq!(svc.decision_for(&p), TrustDecision::Disabled);
    assert!(!svc.is_enabled(&p));
}

#[test]
fn enable_session_is_not_persisted_and_wins_over_disabled() {
    let svc = MacroService::in_memory();
    let p = payload(b"b");
    svc.enable_session(&p);
    assert_eq!(svc.decision_for(&p), TrustDecision::SessionOnly);
    assert!(svc.is_enabled(&p));
    // No persistent record was created.
    assert!(svc.all_records().is_empty());
}

#[test]
fn trust_document_persists_and_enables() {
    let svc = MacroService::in_memory();
    let p = payload(b"c");
    svc.trust_document(&p, None).expect("trust");
    assert_eq!(svc.decision_for(&p), TrustDecision::Trusted);
    assert_eq!(svc.all_records().len(), 1);
}

#[test]
fn keep_disabled_is_sticky_and_clears_session() {
    let svc = MacroService::in_memory();
    let p = payload(b"d");
    svc.enable_session(&p);
    assert!(svc.is_enabled(&p));
    svc.keep_disabled(&p, None).expect("disable");
    assert_eq!(svc.decision_for(&p), TrustDecision::Disabled);
    assert!(!svc.is_enabled(&p));
    // A sticky Disabled record exists for the chip on later opens.
    assert_eq!(svc.all_records().len(), 1);
}

#[test]
fn auto_run_open_defaults_off_and_requires_record() {
    let svc = MacroService::in_memory();
    let p = payload(b"e");
    assert!(!svc.auto_run_open(&p));
    // No record yet → no-op.
    svc.set_auto_run_open(&p, true).expect("noop");
    assert!(!svc.auto_run_open(&p));
    // With trust, the opt-in sticks.
    svc.trust_document(&p, None).expect("trust");
    svc.set_auto_run_open(&p, true).expect("optin");
    assert!(svc.auto_run_open(&p));
}

#[test]
fn network_enabled_defaults_off_and_requires_record() {
    let svc = MacroService::in_memory();
    let p = payload(b"net");
    assert!(!svc.network_enabled(&p));
    // No persistent record yet → the opt-in is a no-op.
    svc.set_allow_network(&p, true).expect("noop");
    assert!(!svc.network_enabled(&p));
    // With trust, the per-document opt-in sticks (and the security snapshot too).
    svc.trust_document(&p, None).expect("trust");
    svc.set_allow_network(&p, true).expect("optin");
    assert!(svc.network_enabled(&p));
    assert!(svc.security_for(&p).allow_network);
    // It is independent of the auto-run opt-in.
    assert!(!svc.auto_run_open(&p));
}

#[test]
fn session_and_always_grants_resolve_into_grant_set() {
    let svc = MacroService::in_memory();
    let p = payload(b"f");
    svc.grant_session(&p, Capability::Print);
    svc.grant_always(&p, Capability::DocWrite).expect("grant");

    let set = svc.grant_set_for(&p);
    assert!(set.contains(Capability::Print));
    assert!(set.contains(Capability::DocWrite));
    // A capability that was never granted is absent.
    assert!(!set.contains(Capability::ClipboardRead));
}

#[test]
fn refused_capability_never_granted() {
    let svc = MacroService::in_memory();
    let p = payload(b"g");
    svc.grant_session(&p, Capability::Network);
    svc.grant_always(&p, Capability::Network).expect("noop");
    assert!(!svc.grant_set_for(&p).contains(Capability::Network));
}

#[test]
fn revoke_removes_persisted_and_session_grants() {
    let svc = MacroService::in_memory();
    let p = payload(b"h");
    svc.grant_always(&p, Capability::DocWrite).expect("grant");
    svc.grant_session(&p, Capability::DocWrite);
    assert!(svc.grant_set_for(&p).contains(Capability::DocWrite));
    svc.revoke(&p, Capability::DocWrite).expect("revoke");
    assert!(!svc.grant_set_for(&p).contains(Capability::DocWrite));
}

#[test]
fn forget_removes_everything() {
    let svc = MacroService::in_memory();
    let p = payload(b"i");
    svc.trust_document(&p, None).expect("trust");
    svc.grant_session(&p, Capability::Print);
    svc.forget(&p).expect("forget");
    assert_eq!(svc.decision_for(&p), TrustDecision::Disabled);
    assert!(svc.all_records().is_empty());
    assert!(!svc.grant_set_for(&p).contains(Capability::Print));
}

#[test]
fn security_summary_reflects_state() {
    let svc = MacroService::in_memory();
    let p = payload(b"j");
    svc.trust_document(&p, None).expect("trust");
    svc.grant_always(&p, Capability::DocWrite).expect("grant");
    svc.grant_session(&p, Capability::UiDialog);

    let sec = svc.security_for(&p);
    assert_eq!(sec.decision, TrustDecision::Trusted);
    assert!(sec.has_record);
    // DocRead (baseline) is omitted from the per-capability rows.
    assert!(
        sec.capabilities
            .iter()
            .all(|c| c.capability != Capability::DocRead)
    );
    let docwrite = sec
        .capabilities
        .iter()
        .find(|c| c.capability == Capability::DocWrite)
        .expect("row");
    assert!(docwrite.persisted && docwrite.granted());
    let dialog = sec
        .capabilities
        .iter()
        .find(|c| c.capability == Capability::UiDialog)
        .expect("row");
    assert!(dialog.session && !dialog.persisted && dialog.granted());
    let net = sec
        .capabilities
        .iter()
        .find(|c| c.capability == Capability::Network)
        .expect("row");
    assert!(net.refused && !net.granted());
}

#[test]
fn reauthor_carries_trust_to_the_edited_payload() {
    let svc = MacroService::in_memory();
    let old = payload(b"Sub Main : End Sub");
    let new = payload(b"Sub Main : Beep : End Sub");
    svc.trust_document(&old, None).unwrap();
    assert_eq!(svc.decision_for(&old), TrustDecision::Trusted);

    svc.reauthor(&old, &new).unwrap();
    // Trust moved to the edited hash; the old hash is untrusted again.
    assert_eq!(svc.decision_for(&new), TrustDecision::Trusted);
    assert_eq!(svc.decision_for(&old), TrustDecision::Disabled);
}

#[test]
fn reauthor_carries_a_session_override() {
    let svc = MacroService::in_memory();
    let old = payload(b"orig");
    let new = payload(b"edited");
    svc.enable_session(&old);
    assert_eq!(svc.decision_for(&old), TrustDecision::SessionOnly);

    svc.reauthor(&old, &new).unwrap();
    assert_eq!(svc.decision_for(&new), TrustDecision::SessionOnly);
    assert_eq!(svc.decision_for(&old), TrustDecision::Disabled);
}

#[test]
fn reauthor_fabricates_no_trust_for_an_untrusted_document() {
    let svc = MacroService::in_memory();
    let old = payload(b"never trusted");
    let new = payload(b"never trusted, edited");
    svc.reauthor(&old, &new).unwrap();
    assert_eq!(svc.decision_for(&new), TrustDecision::Disabled);
}
