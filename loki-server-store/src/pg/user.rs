// SPDX-License-Identifier: Apache-2.0

//! `UserStore` on Postgres.

use async_trait::async_trait;
use loki_model::UserId;
use sqlx::postgres::PgRow;
use sqlx::Row;
use uuid::Uuid;

use crate::error::StoreError;
use crate::ports::UserStore;
use crate::records::UserRecord;

use super::PgStores;

fn user_from_row(row: PgRow) -> Result<UserRecord, StoreError> {
    Ok(UserRecord {
        id: UserId::from_uuid(row.try_get::<Uuid, _>("id")?),
        oidc_sub: row.try_get("oidc_sub")?,
        display_name: row.try_get("display_name")?,
        public_key: row.try_get("public_key")?,
    })
}

#[async_trait]
impl UserStore for PgStores {
    async fn upsert_user_by_oidc(
        &self,
        oidc_sub: &str,
        display_name: &str,
    ) -> Result<UserRecord, StoreError> {
        // Just-in-time provisioning on first login (ADR-C017): identity is
        // whatever the IdP asserts; the display name follows the IdP.
        let row = sqlx::query(
            "INSERT INTO app_user (id, oidc_sub, display_name)
             VALUES ($1, $2, $3)
             ON CONFLICT (oidc_sub) DO UPDATE SET display_name = EXCLUDED.display_name
             RETURNING id, oidc_sub, display_name, public_key",
        )
        .bind(UserId::new().as_uuid())
        .bind(oidc_sub)
        .bind(display_name)
        .fetch_one(self.pool())
        .await?;
        user_from_row(row)
    }

    async fn get_user(&self, id: UserId) -> Result<Option<UserRecord>, StoreError> {
        let row = sqlx::query(
            "SELECT id, oidc_sub, display_name, public_key FROM app_user WHERE id = $1",
        )
        .bind(id.as_uuid())
        .fetch_optional(self.pool())
        .await?;
        row.map(user_from_row).transpose()
    }

    async fn set_public_key(&self, id: UserId, public_key: &[u8]) -> Result<(), StoreError> {
        let result = sqlx::query("UPDATE app_user SET public_key = $2 WHERE id = $1")
            .bind(id.as_uuid())
            .bind(public_key)
            .execute(self.pool())
            .await?;
        if result.rows_affected() == 0 {
            return Err(StoreError::NotFound);
        }
        Ok(())
    }

    async fn anonymize_user(&self, id: UserId) -> Result<(), StoreError> {
        // GDPR erasure: strip personal data but keep the row so document
        // history and audit references stay resolvable (ADR-C020).
        let result = sqlx::query(
            "UPDATE app_user
             SET display_name = '', public_key = NULL, oidc_sub = 'erased:' || id::text
             WHERE id = $1",
        )
        .bind(id.as_uuid())
        .execute(self.pool())
        .await?;
        if result.rows_affected() == 0 {
            return Err(StoreError::NotFound);
        }
        Ok(())
    }
}
