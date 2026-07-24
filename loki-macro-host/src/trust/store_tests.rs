// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 AppThere Loki contributors

use loki_doc_model::io::macros::{MacroPayload, MacroPayloadKind, PreservedPart};

use crate::capability::{Capability, GrantScope};
use crate::trust::{Provenance, TrustDecision, TrustRecord, TrustStore};

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

// ── §2.5: self-authored edits keep trust; external changes do not ────────────

#[test]
fn reauthor_moves_trust_to_the_new_hash_and_marks_authored() {
    // A trusted document is edited in-app: its payload hash changes old → new.
    let old = payload(b"Sub Main : End Sub").payload_hash();
    let new = payload(b"Sub Main : Beep : End Sub").payload_hash();
    assert_ne!(old, new);

    let mut store = TrustStore::default();
    let mut rec = TrustRecord::new(old, TrustDecision::Trusted);
    rec.auto_run_open = true;
    rec.set_grant(Capability::DocWrite, GrantScope::AlwaysForDocument);
    store.insert(rec);

    assert!(store.reauthor(&old, new), "an existing record moves");

    // Old hash no longer resolves; the new hash carries the trust + grants.
    assert_eq!(store.decision(&old), TrustDecision::Disabled);
    let moved = store.get(&new).expect("trust followed the edit");
    assert_eq!(moved.decision, TrustDecision::Trusted);
    assert!(moved.auto_run_open);
    assert!(moved.grants(Capability::DocWrite));
    assert!(
        moved.is_authored(),
        "an in-app edit is self-authored (§2.5)"
    );
    assert_eq!(store.len(), 1, "re-keyed, not duplicated");
}

#[test]
fn reauthor_never_fabricates_trust_for_an_unknown_document() {
    // No record at the old hash ⇒ nothing to carry; trust is never invented.
    let old = payload(b"never trusted").payload_hash();
    let new = payload(b"never trusted, now edited").payload_hash();
    let mut store = TrustStore::default();

    assert!(!store.reauthor(&old, new));
    assert!(store.is_empty());
    assert_eq!(store.decision(&new), TrustDecision::Disabled);
}

#[test]
fn external_modification_without_reauthor_drops_trust() {
    // The contrast to reauthor: a document changed *outside* Loki simply presents
    // a new hash the store has no record for — untrusted, per §2.4. (Nothing
    // calls reauthor for an external change.)
    let original = payload(b"trusted body").payload_hash();
    let tampered = payload(b"trusted body + injected macro").payload_hash();

    let mut store = TrustStore::default();
    store.insert(TrustRecord::new(original, TrustDecision::Trusted));

    assert_eq!(store.decision(&original), TrustDecision::Trusted);
    assert_eq!(
        store.decision(&tampered),
        TrustDecision::Disabled,
        "an external payload change is untrusted until re-enabled"
    );
}

#[test]
fn reauthor_in_place_for_a_noop_edit() {
    // Editing to identical bytes: old_key == new_key. The record stays and is
    // marked authored.
    let key = payload(b"unchanged").payload_hash();
    let mut store = TrustStore::default();
    store.insert(TrustRecord::new(key, TrustDecision::Trusted));

    assert!(store.reauthor(&key, key));
    let rec = store.get(&key).expect("still present");
    assert!(rec.is_authored());
    assert_eq!(rec.decision, TrustDecision::Trusted);
    assert_eq!(store.len(), 1);
}

#[test]
fn authored_provenance_persists_across_save_and_reload() {
    let dir = std::env::temp_dir().join(format!("loki-trust-prov-{}", std::process::id()));
    let path = dir.join("trust.json");
    let _ = std::fs::remove_file(&path);

    let key = payload(b"authored here").payload_hash();
    {
        let mut store = TrustStore::new(Some(path.clone()));
        store.insert(
            TrustRecord::new(key, TrustDecision::Trusted).with_provenance(Provenance::AuthoredHere),
        );
        store.save().expect("save");
    }
    let reloaded = TrustStore::load(path.clone()).expect("load");
    assert!(reloaded.get(&key).expect("record survived").is_authored());

    let _ = std::fs::remove_file(&path);
    let _ = std::fs::remove_dir(&dir);
}

#[test]
fn provenance_defaults_to_external_for_legacy_records() {
    // A record written before the field existed (no `provenance` key) loads as
    // External, so old stores are never silently treated as self-authored.
    let key = [7u8; 32];
    let json = format!(
        r#"{{"version":1,"records":{{"{}":{{"doc_key":"{}","decision":"Trusted"}}}}}}"#,
        crate::trust::hex::encode(&key),
        crate::trust::hex::encode(&key),
    );
    let dir = std::env::temp_dir().join(format!("loki-trust-legacy-{}", std::process::id()));
    let path = dir.join("trust.json");
    std::fs::create_dir_all(&dir).expect("mkdir");
    std::fs::write(&path, json).expect("write");

    let store = TrustStore::load(path.clone()).expect("load");
    assert!(!store.get(&key).expect("loaded").is_authored());

    let _ = std::fs::remove_file(&path);
    let _ = std::fs::remove_dir(&dir);
}
