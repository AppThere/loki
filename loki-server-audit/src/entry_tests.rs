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
