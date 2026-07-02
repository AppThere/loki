// SPDX-License-Identifier: Apache-2.0

//! SQLx/Postgres implementation of the persistence ports (ADR-C016).
//!
//! Queries are runtime-checked (`sqlx::query`) rather than macro-checked so
//! the workspace builds without a live `DATABASE_URL`. Schema migrations are
//! embedded from `migrations/` and applied by [`PgStores::migrate`].

mod audit;
mod document;
mod member;
mod oplog;
mod user;
mod workspace;

use std::sync::Arc;

use loki_crypto::WrappedDek;
use loki_model::{EncryptionTier, Residency, Role};
use sqlx::PgPool;

use crate::error::StoreError;
use crate::ports::Stores;

/// All six ports backed by one Postgres pool.
#[derive(Clone)]
pub struct PgStores {
    pool: PgPool,
}

impl PgStores {
    /// Wraps an existing connection pool.
    #[must_use]
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }

    /// Applies embedded, forward-only migrations (ADR-C016).
    pub async fn migrate(&self) -> Result<(), StoreError> {
        sqlx::migrate!("./migrations")
            .run(&self.pool)
            .await
            .map_err(|e| StoreError::Database(e.into()))
    }

    /// Bundles this instance into a [`Stores`] aggregate.
    #[must_use]
    pub fn into_stores(self) -> Stores {
        Stores {
            workspaces: Arc::new(self.clone()),
            users: Arc::new(self.clone()),
            documents: Arc::new(self.clone()),
            members: Arc::new(self.clone()),
            oplog: Arc::new(self.clone()),
            audit: Arc::new(self),
        }
    }

    pub(crate) fn pool(&self) -> &PgPool {
        &self.pool
    }
}

/// Decodes a stored tier (`smallint`).
pub(crate) fn tier_from_db(value: i16) -> Result<EncryptionTier, StoreError> {
    EncryptionTier::try_from(value).map_err(|e| StoreError::Corrupt(e.to_string()))
}

/// Decodes a stored role (`text`).
pub(crate) fn role_from_db(value: &str) -> Result<Role, StoreError> {
    value
        .parse()
        .map_err(|e: loki_model::RoleParseError| StoreError::Corrupt(e.to_string()))
}

/// Decodes a stored residency (`text`).
pub(crate) fn residency_from_db(value: &str) -> Result<Residency, StoreError> {
    Residency::parse(value).map_err(|e| StoreError::Corrupt(e.to_string()))
}

/// Decodes an optional wrapped DEK (`jsonb`).
pub(crate) fn wrapped_dek_from_db(
    value: Option<serde_json::Value>,
) -> Result<Option<WrappedDek>, StoreError> {
    value
        .map(|v| serde_json::from_value(v).map_err(|e| StoreError::Corrupt(e.to_string())))
        .transpose()
}

/// Encodes an optional wrapped DEK for a `jsonb` column.
pub(crate) fn wrapped_dek_to_db(
    value: Option<&WrappedDek>,
) -> Result<Option<serde_json::Value>, StoreError> {
    value
        .map(|w| serde_json::to_value(w).map_err(|e| StoreError::Corrupt(e.to_string())))
        .transpose()
}
