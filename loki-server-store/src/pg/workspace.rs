// SPDX-License-Identifier: Apache-2.0

//! `WorkspaceStore` on Postgres.

use async_trait::async_trait;
use loki_model::WorkspaceId;
use sqlx::Row;
use uuid::Uuid;

use crate::error::StoreError;
use crate::ports::WorkspaceStore;
use crate::records::{DocMetaRecord, WorkspaceRecord};

use super::document::doc_meta_from_row;
use super::{residency_from_db, tier_from_db, PgStores};

#[async_trait]
impl WorkspaceStore for PgStores {
    async fn create_workspace(&self, workspace: &WorkspaceRecord) -> Result<(), StoreError> {
        sqlx::query(
            "INSERT INTO workspace (id, name, default_tier, residency, created_at)
             VALUES ($1, $2, $3, $4, $5)",
        )
        .bind(workspace.id.as_uuid())
        .bind(&workspace.name)
        .bind(workspace.default_tier.as_i16())
        .bind(workspace.residency.as_config_value())
        .bind(workspace.created_at)
        .execute(self.pool())
        .await?;
        Ok(())
    }

    async fn get_workspace(&self, id: WorkspaceId) -> Result<Option<WorkspaceRecord>, StoreError> {
        let row = sqlx::query(
            "SELECT id, name, default_tier, residency, created_at
             FROM workspace WHERE id = $1",
        )
        .bind(id.as_uuid())
        .fetch_optional(self.pool())
        .await?;
        row.map(|row| {
            Ok(WorkspaceRecord {
                id: WorkspaceId::from_uuid(row.try_get::<Uuid, _>("id")?),
                name: row.try_get("name")?,
                default_tier: tier_from_db(row.try_get("default_tier")?)?,
                residency: residency_from_db(row.try_get::<String, _>("residency")?.as_str())?,
                created_at: row.try_get("created_at")?,
            })
        })
        .transpose()
    }

    async fn list_documents(&self, id: WorkspaceId) -> Result<Vec<DocMetaRecord>, StoreError> {
        let rows = sqlx::query(
            "SELECT id, workspace_id, title, tier, residency, snapshot_ptr, dek_wrapped,
                    created_at
             FROM doc_meta WHERE workspace_id = $1 ORDER BY created_at DESC",
        )
        .bind(id.as_uuid())
        .fetch_all(self.pool())
        .await?;
        rows.into_iter().map(doc_meta_from_row).collect()
    }
}
