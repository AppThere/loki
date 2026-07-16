// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 AppThere Loki contributors

use loki_doc_model::io::macros::{MacroPayload, MacroPayloadKind, PreservedPart};

use crate::capability::{Capability, GrantScope};
use crate::trust::{TrustDecision, TrustRecord, TrustStore};

fn payload(bytes: &[u8]) -> MacroPayload {
    MacroPayload::new(
        MacroPayloadKind::OoxmlVba,
        vec![PreservedPart::new(
            "/word/vbaProject.bin",
            None,
            bytes.to_vec(),
        )],
    )
}

// ── T10: trust never comes from the document ─────────────────────────────────

#[test]
fn t10_fresh_store_trusts_nothing() {
    // A document that "claims" trust in its own bytes still has no record.
    let claims_trust = payload(b"decision=Trusted auto_run_open=true");
    let store = TrustStore::default();
    assert_eq!(
        store.decision(&claims_trust.payload_hash()),
        TrustDecision::Disabled,
        "trust must never be inferred from document content"
    );
    assert!(store.get(&claims_trust.payload_hash()).is_none());
}

#[test]
fn t10_trust_is_keyed_by_payload_hash_only() {
    let a = payload(b"Sub Foo() End Sub");
    let mut store = TrustStore::default();
    store.insert(TrustRecord::new(a.payload_hash(), TrustDecision::Trusted));
    assert_eq!(store.decision(&a.payload_hash()), TrustDecision::Trusted);

    // A different payload (even one byte) is a different key → untrusted.
    let b = payload(b"Sub Foo() End Sub ");
    assert_ne!(a.payload_hash(), b.payload_hash());
    assert_eq!(store.decision(&b.payload_hash()), TrustDecision::Disabled);
}

#[test]
fn t10_changing_macros_revokes_trust_by_key_mismatch() {
    // Trust the original; editing the macros yields a new hash the store has
    // never seen, so the edited document is untrusted (spec §2.2).
    let original = payload(b"MsgBox \"hi\"");
    let mut store = TrustStore::default();
    store.insert(TrustRecord::new(
        original.payload_hash(),
        TrustDecision::Trusted,
    ));

    let tampered = payload(b"Shell \"calc.exe\"");
    assert_eq!(
        store.decision(&tampered.payload_hash()),
        TrustDecision::Disabled
    );
}

// ── Record + grant behaviour ─────────────────────────────────────────────────

#[test]
fn grants_are_set_and_revoked() {
    let p = payload(b"x");
    let mut rec = TrustRecord::new(p.payload_hash(), TrustDecision::Trusted);
    assert!(!rec.grants(Capability::DocWrite));

    rec.set_grant(Capability::DocWrite, GrantScope::AlwaysForDocument);
    assert!(rec.grants(Capability::DocWrite));

    // Non-persistent scopes are not stored on the record.
    rec.set_grant(Capability::Print, GrantScope::AllowSession);
    assert!(!rec.grants(Capability::Print));

    rec.revoke(Capability::DocWrite);
    assert!(!rec.grants(Capability::DocWrite));
}

// ── Persistence ──────────────────────────────────────────────────────────────

#[test]
fn save_and_reload_roundtrips_persistent_records() {
    let dir = std::env::temp_dir().join(format!("loki-trust-{}", std::process::id()));
    let path = dir.join("trust.json");
    let _ = std::fs::remove_file(&path);

    let p = payload(b"Sub AutoOpen() End Sub");
    let key = p.payload_hash();
    {
        let mut store = TrustStore::new(Some(path.clone()));
        let mut rec = TrustRecord::new(key, TrustDecision::Trusted);
        rec.auto_run_open = true;
        rec.set_grant(Capability::DocWrite, GrantScope::AlwaysForDocument);
        store.insert(rec);
        store.save().expect("save");
    }
    let reloaded = TrustStore::load(path.clone()).expect("load");
    let rec = reloaded.get(&key).expect("record survived reload");
    assert_eq!(rec.decision, TrustDecision::Trusted);
    assert!(rec.auto_run_open);
    assert!(rec.grants(Capability::DocWrite));

    let _ = std::fs::remove_file(&path);
    let _ = std::fs::remove_dir(&dir);
}

#[test]
fn session_only_records_are_not_persisted() {
    let dir = std::env::temp_dir().join(format!("loki-trust-sess-{}", std::process::id()));
    let path = dir.join("trust.json");
    let _ = std::fs::remove_file(&path);

    let p = payload(b"session doc");
    let key = p.payload_hash();
    {
        let mut store = TrustStore::new(Some(path.clone()));
        store.insert(TrustRecord::new(key, TrustDecision::SessionOnly));
        store.save().expect("save");
    }
    let reloaded = TrustStore::load(path.clone()).expect("load");
    assert!(
        reloaded.get(&key).is_none(),
        "session-only trust must never reach disk"
    );

    let _ = std::fs::remove_file(&path);
    let _ = std::fs::remove_dir(&dir);
}

#[test]
fn load_missing_file_is_empty_not_error() {
    let path = std::env::temp_dir().join("loki-trust-does-not-exist-xyz.json");
    let _ = std::fs::remove_file(&path);
    let store = TrustStore::load(path).expect("missing file is not an error");
    assert!(store.is_empty());
}

#[test]
fn corrupt_file_degrades_to_empty_with_load_or_empty() {
    let dir = std::env::temp_dir().join(format!("loki-trust-corrupt-{}", std::process::id()));
    let path = dir.join("trust.json");
    std::fs::create_dir_all(&dir).expect("mkdir");
    std::fs::write(&path, b"{ this is not json").expect("write");

    assert!(TrustStore::load(path.clone()).is_err());
    let store = TrustStore::load_or_empty(path.clone());
    assert!(store.is_empty());

    let _ = std::fs::remove_file(&path);
    let _ = std::fs::remove_dir(&dir);
}

#[test]
fn forget_removes_record() {
    let p = payload(b"forget me");
    let key = p.payload_hash();
    let mut store = TrustStore::default();
    store.insert(TrustRecord::new(key, TrustDecision::Trusted));
    assert_eq!(store.len(), 1);
    assert!(store.forget(&key).is_some());
    assert!(store.is_empty());
}
