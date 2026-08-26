// SPDX-License-Identifier: Apache-2.0

//! Tamper-evidence tests for the audit chain.

use chrono::TimeZone;

use super::*;

fn sample_chain(len: usize) -> Vec<AuditEntry> {
    let mut chain: Vec<AuditEntry> = Vec::with_capacity(len);
    for i in 0..len {
        let at = chrono::Utc
            .timestamp_micros(1_750_000_000_000_000 + i as i64)
            .single()
            .unwrap();
        let entry = AuditEntry::append(
            chain.last(),
            format!("user-{i}"),
            AuditAction::AclChange,
            format!("doc-{i}"),
            at,
        );
        chain.push(entry);
    }
    chain
}

#[test]
fn valid_chain_verifies() {
    assert_eq!(verify_chain(&[]), Ok(()));
    assert_eq!(verify_chain(&sample_chain(5)), Ok(()));
}

#[test]
fn mutated_field_is_detected() {
    let mut chain = sample_chain(4);
    chain[2].target = "doc-other".to_owned();
    assert_eq!(
        verify_chain(&chain),
        Err(ChainError::HashMismatch { seq: 3 })
    );
}

#[test]
fn removed_entry_is_detected() {
    let mut chain = sample_chain(4);
    chain.remove(1);
    assert_eq!(
        verify_chain(&chain),
        Err(ChainError::OutOfSequence {
            seq: 3,
            expected: 2
        })
    );
}

#[test]
fn replaced_entry_breaks_the_link() {
    let mut chain = sample_chain(4);
    // Rebuild entry 2 with different content and a *recomputed* hash, but
    // without re-linking the rest of the chain — the successor detects it.
    let at = chain[1].created_at;
    chain[1] = AuditEntry::append(Some(&chain[0]), "mallory", AuditAction::Delete, "doc-1", at);
    assert_eq!(verify_chain(&chain), Err(ChainError::BrokenLink { seq: 3 }));
}

#[test]
fn chain_must_start_at_genesis() {
    let chain = sample_chain(3);
    assert_eq!(verify_chain(&chain[1..]), Err(ChainError::BadGenesis));
}

#[test]
fn timestamp_mutation_is_detected() {
    let mut chain = sample_chain(2);
    chain[1].created_at += chrono::Duration::seconds(1);
    assert_eq!(
        verify_chain(&chain),
        Err(ChainError::HashMismatch { seq: 2 })
    );
}

// ---------------------------------------------------------------------------
// Known-answer vector for `compute_hash` (F-AU-1).
//
// Every test above builds a chain with `append` and checks it with
// `verify_chain` — the same `compute_hash` on both sides. That is blind to a
// coordinated change: alter the `b"loki-audit.v1"` domain separator, the field
// order, or the timestamp precision and the whole suite still passes, while
// every chain already persisted in `audit_log` becomes unverifiable (and the
// tamper-evidence claim of ADR-C020 quietly becomes false for historical
// rows).
//
// PROVENANCE: the two hashes below were captured by running the current
// implementation once on 2026-08-25 and pasting its output. They are not
// derived from an independent SHA-256 reimplementation. Their purpose is to
// freeze today's canonical encoding. A failure here means the encoding
// changed: existing rows must be re-hashed (or the chain re-anchored) as part
// of that change — re-capturing the constant hides exactly the event this
// test exists to report.
// ---------------------------------------------------------------------------

/// Hex-encodes a hash for readable assertion failures.
fn hex(bytes: &[u8]) -> String {
    bytes.iter().map(|b| format!("{b:02x}")).collect()
}

/// Fixed instant for the vectors: 2025-08-25T13:24:16.789012Z.
const KAT_MICROS: i64 = 1_756_123_456_789_012;
/// Hash of the genesis entry `(seq 1, zero prev_hash, "user-42", acl-change,
/// "doc-7", KAT_MICROS)`.
const KAT_GENESIS_HASH: &str = "f270ff40489d564228b9ea311ca6368605f23655e74eab77b3eb3e2c0cb6ca2a";
/// Hash of the successor `(seq 2, prev = KAT_GENESIS_HASH, "system",
/// gdpr-erase, "user-9", KAT_MICROS + 1)`.
const KAT_SECOND_HASH: &str = "3ccc4cd3043d13b21896c1d1881ff2b522cd296beed85e8ce26954b2b4ceae12";

fn at(micros: i64) -> DateTime<Utc> {
    chrono::Utc.timestamp_micros(micros).single().unwrap()
}

#[test]
fn compute_hash_matches_the_pinned_vectors() {
    let genesis = AuditEntry::append(
        None,
        "user-42",
        AuditAction::AclChange,
        "doc-7",
        at(KAT_MICROS),
    );
    assert_eq!(genesis.seq, 1);
    assert_eq!(genesis.prev_hash, [0u8; HASH_LEN]);
    assert_eq!(hex(&genesis.hash), KAT_GENESIS_HASH);

    let second = AuditEntry::append(
        Some(&genesis),
        "system",
        AuditAction::GdprErase,
        "user-9",
        at(KAT_MICROS + 1),
    );
    assert_eq!(second.seq, 2);
    assert_eq!(hex(&second.prev_hash), KAT_GENESIS_HASH);
    assert_eq!(hex(&second.hash), KAT_SECOND_HASH);

    // The pinned entries must also be accepted by the verifier — a vector that
    // pinned a hash the verifier rejects would be pinning the wrong thing.
    assert_eq!(verify_chain(&[genesis, second]), Ok(()));
}

#[test]
fn hash_input_uses_microsecond_timestamp_precision() {
    // `compute_hash` feeds `created_at.timestamp_micros()`. Coarser precision
    // (millis) would stop distinguishing entries 1 µs apart; finer (nanos)
    // would make sub-microsecond noise part of the hash, so a value read back
    // from Postgres `timestamptz` (microsecond resolution) would no longer
    // reproduce the stored hash. Both directions are asserted.
    let base = chrono::Utc.timestamp_nanos(KAT_MICROS * 1_000);
    let within_same_micro = chrono::Utc.timestamp_nanos(KAT_MICROS * 1_000 + 999);
    let next_micro = at(KAT_MICROS + 1);

    let entry = |t| AuditEntry::append(None, "a", AuditAction::Export, "b", t);
    assert_eq!(
        entry(base).hash,
        entry(within_same_micro).hash,
        "sub-microsecond precision leaked into the hash input"
    );
    assert_ne!(
        entry(base).hash,
        entry(next_micro).hash,
        "timestamp precision is coarser than a microsecond"
    );
}

// ---------------------------------------------------------------------------
// The length-prefix forgery defense (F-AU-2).
// ---------------------------------------------------------------------------

/// The encoding the u64 length prefixes exist to prevent: variable-length
/// fields concatenated with no boundary marker. Deliberately *not* a mirror of
/// `compute_hash` — it is the counterexample, kept minimal on purpose.
fn unprefixed_digest(actor: &str, action: AuditAction, target: &str) -> [u8; HASH_LEN] {
    let mut hasher = Sha256::new();
    for field in [actor, action.as_str(), target] {
        hasher.update(field.as_bytes());
    }
    hasher.finalize().into()
}

#[test]
fn length_prefixes_stop_field_boundaries_from_being_shifted() {
    // All three entries have identical seq, prev_hash, action and timestamp,
    // and their actor||action||target byte strings are identical
    // ("" + "delete" + "deletedelete" == "delete" + "delete" + "delete" ==
    //  "deletedelete" + "delete" + ""). Only the boundaries move — an
    // attacker moving text between the "who" and the "what to" columns.
    let when = at(KAT_MICROS);
    let split = |actor: &str, target: &str| {
        AuditEntry::append(None, actor, AuditAction::Delete, target, when)
    };
    let a = split("", "deletedelete");
    let b = split("delete", "delete");
    let c = split("deletedelete", "");

    // First establish that the hazard is real: with no length prefixes these
    // three distinct records share one encoding, so the collision is
    // reachable and this test is not asserting a vacuous property.
    let flat = unprefixed_digest("", AuditAction::Delete, "deletedelete");
    assert_eq!(
        flat,
        unprefixed_digest("delete", AuditAction::Delete, "delete")
    );
    assert_eq!(
        flat,
        unprefixed_digest("deletedelete", AuditAction::Delete, "")
    );

    // The canonical encoding must separate them.
    assert_ne!(a.hash, b.hash, "actor/target boundary is not hashed");
    assert_ne!(b.hash, c.hash, "actor/target boundary is not hashed");
    assert_ne!(a.hash, c.hash, "actor/target boundary is not hashed");
}
