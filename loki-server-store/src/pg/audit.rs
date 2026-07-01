// SPDX-License-Identifier: Apache-2.0

//! `AuditStore` on Postgres (ADR-C020).
//!
//! Appends run inside a transaction holding an advisory lock, so the hash
//! chain stays linear even with multiple `loki-server` replicas writing to
//! the same database.

use async_trait::async_trait;
use chrono::Utc;
use loki_server_audit::{AuditAction, AuditEntry, HASH_LEN};
use sqlx::postgres::PgRow;
use sqlx::Row;

use crate::error::StoreError;
use crate::ports::AuditStore;

use super::PgStores;

/// Advisory-lock key for audit appends ("loki" in ASCII).
const AUDIT_LOCK_KEY: i64 = 0x6c6f_6b69;

fn hash_from_db(bytes: Vec<u8>) -> Result<[u8; HASH_LEN], StoreError> {
    bytes
        .try_into()
        .map_err(|b: Vec<u8>| StoreError::Corrupt(format!("audit hash of {} bytes", b.len())))
}

fn entry_from_row(row: PgRow) -> Result<AuditEntry, StoreError> {
    let seq: i64 = row.try_get("seq")?;
    let seq = u64::try_from(seq)
        .map_err(|_| StoreError::Corrupt(format!("negative audit seq {seq}")))?;
    let action: String = row.try_get("action")?;
    Ok(AuditEntry {
        seq,
        prev_hash: hash_from_db(row.try_get("prev_hash")?)?,
        hash: hash_from_db(row.try_get("hash")?)?,
        actor: row.try_get("actor")?,
        action: action
            .parse()
            .map_err(|e: loki_server_audit::ActionParseError| StoreError::Corrupt(e.to_string()))?,
        target: row.try_get("target")?,
        created_at: row.try_get("created_at")?,
    })
}

#[async_trait]
impl AuditStore for PgStores {
    async fn append_audit(
        &self,
        actor: &str,
        action: AuditAction,
        target: &str,
    ) -> Result<AuditEntry, StoreError> {
        let mut tx = self.pool().begin().await?;
        // Serialize appends across replicas; released at commit/rollback.
        sqlx::query("SELECT pg_advisory_xact_lock($1)")
            .bind(AUDIT_LOCK_KEY)
            .execute(&mut *tx)
            .await?;
        let head = sqlx::query(
            "SELECT seq, prev_hash, hash, actor, action, target, created_at
             FROM audit_log ORDER BY seq DESC LIMIT 1",
        )
        .fetch_optional(&mut *tx)
        .await?
        .map(entry_from_row)
        .transpose()?;
        let entry = AuditEntry::append(head.as_ref(), actor, action, target, Utc::now());
        sqlx::query(
            "INSERT INTO audit_log (seq, prev_hash, hash, actor, action, target, created_at)
             VALUES ($1, $2, $3, $4, $5, $6, $7)",
        )
        .bind(i64::try_from(entry.seq).map_err(|_| {
            StoreError::Corrupt(format!("audit seq {} exceeds i64", entry.seq))
        })?)
        .bind(entry.prev_hash.as_slice())
        .bind(entry.hash.as_slice())
        .bind(&entry.actor)
        .bind(entry.action.as_str())
        .bind(&entry.target)
        .bind(entry.created_at)
        .execute(&mut *tx)
        .await?;
        tx.commit().await?;
        Ok(entry)
    }

    async fn load_chain(&self) -> Result<Vec<AuditEntry>, StoreError> {
        let rows = sqlx::query(
            "SELECT seq, prev_hash, hash, actor, action, target, created_at
             FROM audit_log ORDER BY seq",
        )
        .fetch_all(self.pool())
        .await?;
        rows.into_iter().map(entry_from_row).collect()
    }
}
