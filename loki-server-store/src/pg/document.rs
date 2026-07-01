// SPDX-License-Identifier: Apache-2.0

//! `DocumentStore` on Postgres.

use async_trait::async_trait;
use loki_crypto::WrappedDek;
use loki_model::{DocumentId, EncryptionTier, WorkspaceId};
use sqlx::postgres::PgRow;
use sqlx::Row;
use uuid::Uuid;

use crate::error::StoreError;
use crate::ports::DocumentStore;
use crate::records::DocMetaRecord;

use super::{residency_from_db, tier_from_db, wrapped_dek_from_db, wrapped_dek_to_db, PgStores};

/// Maps a `doc_meta` row (shared with the workspace listing).
pub(super) fn doc_meta_from_row(row: PgRow) -> Result<DocMetaRecord, StoreError> {
    Ok(DocMetaRecord {
        id: DocumentId::from_uuid(row.try_get::<Uuid, _>("id")?),
        workspace_id: WorkspaceId::from_uuid(row.try_get::<Uuid, _>("workspace_id")?),
        title: row.try_get("title")?,
        tier: tier_from_db(row.try_get("tier")?)?,
        residency: residency_from_db(row.try_get::<String, _>("residency")?.as_str())?,
        snapshot_ptr: row.try_get("snapshot_ptr")?,
        dek_wrapped: wrapped_dek_from_db(row.try_get("dek_wrapped")?)?,
        created_at: row.try_get("created_at")?,
    })
}

#[async_trait]
impl DocumentStore for PgStores {
    async fn create_document(&self, doc: &DocMetaRecord) -> Result<(), StoreError> {
        sqlx::query(
            "INSERT INTO doc_meta
                 (id, workspace_id, title, tier, residency, snapshot_ptr, dek_wrapped,
                  created_at)
             VALUES ($1, $2, $3, $4, $5, $6, $7, $8)",
        )
        .bind(doc.id.as_uuid())
        .bind(doc.workspace_id.as_uuid())
        .bind(&doc.title)
        .bind(doc.tier.as_i16())
        .bind(doc.residency.as_config_value())
        .bind(&doc.snapshot_ptr)
        .bind(wrapped_dek_to_db(doc.dek_wrapped.as_ref())?)
        .bind(doc.created_at)
        .execute(self.pool())
        .await?;
        Ok(())
    }

    async fn get_document(&self, id: DocumentId) -> Result<Option<DocMetaRecord>, StoreError> {
        let row = sqlx::query(
            "SELECT id, workspace_id, title, tier, residency, snapshot_ptr, dek_wrapped,
                    created_at
             FROM doc_meta WHERE id = $1",
        )
        .bind(id.as_uuid())
        .fetch_optional(self.pool())
        .await?;
        row.map(doc_meta_from_row).transpose()
    }

    async fn set_snapshot_ptr(&self, id: DocumentId, ptr: &str) -> Result<(), StoreError> {
        let result = sqlx::query("UPDATE doc_meta SET snapshot_ptr = $2 WHERE id = $1")
            .bind(id.as_uuid())
            .bind(ptr)
            .execute(self.pool())
            .await?;
        if result.rows_affected() == 0 {
            return Err(StoreError::NotFound);
        }
        Ok(())
    }

    async fn set_tier(
        &self,
        id: DocumentId,
        tier: EncryptionTier,
        dek_wrapped: Option<&WrappedDek>,
    ) -> Result<(), StoreError> {
        let result = sqlx::query("UPDATE doc_meta SET tier = $2, dek_wrapped = $3 WHERE id = $1")
            .bind(id.as_uuid())
            .bind(tier.as_i16())
            .bind(wrapped_dek_to_db(dek_wrapped)?)
            .execute(self.pool())
            .await?;
        if result.rows_affected() == 0 {
            return Err(StoreError::NotFound);
        }
        Ok(())
    }

    async fn delete_document(&self, id: DocumentId) -> Result<(), StoreError> {
        // Members and oplog rows cascade (FK ON DELETE CASCADE).
        let result = sqlx::query("DELETE FROM doc_meta WHERE id = $1")
            .bind(id.as_uuid())
            .execute(self.pool())
            .await?;
        if result.rows_affected() == 0 {
            return Err(StoreError::NotFound);
        }
        Ok(())
    }

    async fn shred_dek(&self, id: DocumentId) -> Result<(), StoreError> {
        // Destroy every wrapped copy atomically (ADR-C020 crypto-shredding).
        let mut tx = self.pool().begin().await?;
        let result = sqlx::query("UPDATE doc_meta SET dek_wrapped = NULL WHERE id = $1")
            .bind(id.as_uuid())
            .execute(&mut *tx)
            .await?;
        if result.rows_affected() == 0 {
            return Err(StoreError::NotFound);
        }
        sqlx::query("UPDATE doc_member SET dek_wrapped_for_user = NULL WHERE doc_id = $1")
            .bind(id.as_uuid())
            .execute(&mut *tx)
            .await?;
        tx.commit().await?;
        Ok(())
    }
}
