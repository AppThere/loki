// SPDX-License-Identifier: Apache-2.0

//! Typed store errors.

/// Errors surfaced by the persistence layer.
#[derive(Debug, thiserror::Error)]
pub enum StoreError {
    /// A database operation failed.
    #[error("database error: {0}")]
    Database(#[from] sqlx::Error),
    /// An object-storage operation failed.
    #[error("object storage error: {0}")]
    ObjectStore(#[from] object_store::Error),
    /// The requested entity does not exist.
    #[error("not found")]
    NotFound,
    /// A stored value could not be decoded (bad tier, role, JSON, …).
    ///
    /// This indicates data written by an incompatible version or manual
    /// tampering — it is never returned for user input.
    #[error("corrupt stored value: {0}")]
    Corrupt(String),
}
