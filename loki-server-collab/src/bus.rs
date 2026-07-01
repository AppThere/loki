// SPDX-License-Identifier: Apache-2.0

//! The fan-out bus port (ADR-C012).

use async_trait::async_trait;
use loki_model::DocumentId;
use tokio::sync::broadcast;
use uuid::Uuid;

use crate::msg::CollabFrame;

/// Identifies where an event entered the system, so a connection can skip
/// its own echoes and an instance can skip its own `NOTIFY`s.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Origin {
    /// The `loki-server` instance (random per process).
    pub instance: Uuid,
    /// The WebSocket connection within that instance.
    pub conn: u64,
}

/// An event delivered to local subscribers of a document.
#[derive(Debug, Clone)]
pub struct BusEvent {
    /// The document the event belongs to.
    pub doc: DocumentId,
    /// Where the event entered the system.
    pub origin: Origin,
    /// The frame to forward.
    pub frame: CollabFrame,
}

/// Cross-instance fan-out for collaboration events (ADR-C012).
///
/// Implementations must deliver published events to local subscribers of the
/// same document *and* to subscribers on other instances. Updates are already
/// durable in the oplog when published — the bus is delivery, not storage.
#[async_trait]
pub trait FanOutBus: Send + Sync {
    /// Publishes a persisted Loro update (`seq` is its oplog sequence).
    async fn publish_update(
        &self,
        origin: Origin,
        doc: DocumentId,
        seq: i64,
        payload: &[u8],
    ) -> Result<(), BusError>;

    /// Publishes an ephemeral awareness payload (never persisted).
    async fn publish_awareness(
        &self,
        origin: Origin,
        doc: DocumentId,
        payload: &[u8],
    ) -> Result<(), BusError>;

    /// Subscribes to events for one document.
    async fn subscribe(&self, doc: DocumentId) -> broadcast::Receiver<BusEvent>;
}

/// Fan-out failures.
#[derive(Debug, thiserror::Error)]
pub enum BusError {
    /// The underlying transport failed (e.g. the Postgres connection).
    #[error("bus transport error: {0}")]
    Transport(#[from] sqlx::Error),
    /// An awareness payload exceeds the `NOTIFY` size budget and cannot be
    /// fanned out cross-instance (updates never hit this: they travel by
    /// oplog sequence number).
    #[error("awareness payload of {0} bytes exceeds the cross-instance limit")]
    AwarenessTooLarge(usize),
    /// A cross-instance notification could not be decoded.
    #[error("malformed bus notification: {0}")]
    MalformedNotification(String),
    /// The oplog lookup for a notified update failed.
    #[error("oplog fetch for notified update failed: {0}")]
    Oplog(#[from] loki_server_store::StoreError),
}
