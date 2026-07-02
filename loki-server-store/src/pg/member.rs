// SPDX-License-Identifier: Apache-2.0

//! `MemberStore` on Postgres.

use async_trait::async_trait;
use loki_model::{DocumentId, Role, UserId};
use sqlx::Row;
use uuid::Uuid;

use crate::error::StoreError;
use crate::ports::MemberStore;
use crate::records::DocMemberRecord;

use super::{PgStores, role_from_db, wrapped_dek_from_db, wrapped_dek_to_db};

#[async_trait]
impl MemberStore for PgStores {
    async fn upsert_member(&self, member: &DocMemberRecord) -> Result<(), StoreError> {
        sqlx::query(
            "INSERT INTO doc_member (doc_id, user_id, role, dek_wrapped_for_user)
             VALUES ($1, $2, $3, $4)
             ON CONFLICT (doc_id, user_id)
             DO UPDATE SET role = EXCLUDED.role,
                           dek_wrapped_for_user = EXCLUDED.dek_wrapped_for_user",
        )
        .bind(member.doc_id.as_uuid())
        .bind(member.user_id.as_uuid())
        .bind(member.role.as_str())
        .bind(wrapped_dek_to_db(member.dek_wrapped_for_user.as_ref())?)
        .execute(self.pool())
        .await?;
        Ok(())
    }

    async fn get_member_role(
        &self,
        doc: DocumentId,
        user: UserId,
    ) -> Result<Option<Role>, StoreError> {
        let row = sqlx::query("SELECT role FROM doc_member WHERE doc_id = $1 AND user_id = $2")
            .bind(doc.as_uuid())
            .bind(user.as_uuid())
            .fetch_optional(self.pool())
            .await?;
        row.map(|row| role_from_db(row.try_get::<String, _>("role")?.as_str()))
            .transpose()
    }

    async fn list_members(&self, doc: DocumentId) -> Result<Vec<DocMemberRecord>, StoreError> {
        let rows = sqlx::query(
            "SELECT doc_id, user_id, role, dek_wrapped_for_user
             FROM doc_member WHERE doc_id = $1 ORDER BY user_id",
        )
        .bind(doc.as_uuid())
        .fetch_all(self.pool())
        .await?;
        rows.into_iter()
            .map(|row| {
                Ok(DocMemberRecord {
                    doc_id: DocumentId::from_uuid(row.try_get::<Uuid, _>("doc_id")?),
                    user_id: UserId::from_uuid(row.try_get::<Uuid, _>("user_id")?),
                    role: role_from_db(row.try_get::<String, _>("role")?.as_str())?,
                    dek_wrapped_for_user: wrapped_dek_from_db(
                        row.try_get("dek_wrapped_for_user")?,
                    )?,
                })
            })
            .collect()
    }

    async fn remove_member(&self, doc: DocumentId, user: UserId) -> Result<(), StoreError> {
        let result = sqlx::query("DELETE FROM doc_member WHERE doc_id = $1 AND user_id = $2")
            .bind(doc.as_uuid())
            .bind(user.as_uuid())
            .execute(self.pool())
            .await?;
        if result.rows_affected() == 0 {
            return Err(StoreError::NotFound);
        }
        Ok(())
    }
}
