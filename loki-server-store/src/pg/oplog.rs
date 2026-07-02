// SPDX-License-Identifier: Apache-2.0

//! `OplogStore` on Postgres (the hot update log, ADR-C013).

use async_trait::async_trait;
use loki_model::{DocumentId, UserId};
use sqlx::Row;
use sqlx::postgres::PgRow;
use uuid::Uuid;

use crate::error::StoreError;
use crate::ports::OplogStore;
use crate::records::OplogEntry;

use super::PgStores;

fn oplog_from_row(row: PgRow) -> Result<OplogEntry, StoreError> {
    Ok(OplogEntry {
        doc_id: DocumentId::from_uuid(row.try_get::<Uuid, _>("doc_id")?),
        seq: row.try_get("seq")?,
        actor: UserId::from_uuid(row.try_get::<Uuid, _>("actor")?),
        payload: row.try_get("payload")?,
        created_at: row.try_get("created_at")?,
    })
}

#[async_trait]
impl OplogStore for PgStores {
    async fn append(
        &self,
        doc: DocumentId,
        actor: UserId,
        payload: &[u8],
    ) -> Result<i64, StoreError> {
        let row = sqlx::query(
            "INSERT INTO doc_oplog (doc_id, actor, payload) VALUES ($1, $2, $3) RETURNING seq",
        )
        .bind(doc.as_uuid())
        .bind(actor.as_uuid())
        .bind(payload)
        .fetch_one(self.pool())
        .await?;
        Ok(row.try_get("seq")?)
    }

    async fn fetch_after(
        &self,
        doc: DocumentId,
        after: i64,
    ) -> Result<Vec<OplogEntry>, StoreError> {
        let rows = sqlx::query(
            "SELECT doc_id, seq, actor, payload, created_at
             FROM doc_oplog WHERE doc_id = $1 AND seq > $2 ORDER BY seq",
        )
        .bind(doc.as_uuid())
        .bind(after)
        .fetch_all(self.pool())
        .await?;
        rows.into_iter().map(oplog_from_row).collect()
    }

    async fn fetch_one(&self, doc: DocumentId, seq: i64) -> Result<Option<OplogEntry>, StoreError> {
        let row = sqlx::query(
            "SELECT doc_id, seq, actor, payload, created_at
             FROM doc_oplog WHERE doc_id = $1 AND seq = $2",
        )
        .bind(doc.as_uuid())
        .bind(seq)
        .fetch_optional(self.pool())
        .await?;
        row.map(oplog_from_row).transpose()
    }

    async fn truncate_up_to(&self, doc: DocumentId, up_to: i64) -> Result<(), StoreError> {
        // Called after a snapshot is durably written (ADR-C013 compaction).
        sqlx::query("DELETE FROM doc_oplog WHERE doc_id = $1 AND seq <= $2")
            .bind(doc.as_uuid())
            .bind(up_to)
            .execute(self.pool())
            .await?;
        Ok(())
    }

    async fn docs_with_backlog(
        &self,
        min_entries: i64,
    ) -> Result<Vec<(DocumentId, i64)>, StoreError> {
        let rows = sqlx::query(
            "SELECT doc_id, COUNT(*) AS backlog
             FROM doc_oplog GROUP BY doc_id HAVING COUNT(*) >= $1
             ORDER BY backlog DESC",
        )
        .bind(min_entries)
        .fetch_all(self.pool())
        .await?;
        rows.into_iter()
            .map(|row| {
                Ok((
                    DocumentId::from_uuid(row.try_get::<Uuid, _>("doc_id")?),
                    row.try_get::<i64, _>("backlog")?,
                ))
            })
            .collect()
    }
}
