// SPDX-License-Identifier: Apache-2.0

//! The per-connection relay logic, factored out of the WebSocket adapter so
//! it is testable without sockets.

use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;

use loki_model::{DocumentId, UserId};
use loki_server_store::{OplogStore, StoreError};
use uuid::Uuid;

use crate::bus::{BusError, BusEvent, FanOutBus, Origin};
use crate::msg::CollabFrame;

/// Shared collaboration state: one per `loki-server` process.
#[derive(Clone)]
pub struct CollabState {
    /// Oplog persistence (updates are durable before broadcast, ADR-C013).
    pub oplog: Arc<dyn OplogStore>,
    /// Cross-instance fan-out (ADR-C012).
    pub bus: Arc<dyn FanOutBus>,
    /// This process' identity on the bus.
    pub instance: Uuid,
    next_conn: Arc<AtomicU64>,
}

impl CollabState {
    /// Creates process-wide collaboration state.
    #[must_use]
    pub fn new(oplog: Arc<dyn OplogStore>, bus: Arc<dyn FanOutBus>, instance: Uuid) -> Self {
        Self {
            oplog,
            bus,
            instance,
            next_conn: Arc::new(AtomicU64::new(1)),
        }
    }

    /// Opens a relay for one authenticated, authorized connection.
    ///
    /// RBAC happens *before* this call (in the API layer): `can_write` is
    /// `Role::allows(Action::WriteContent)` for the connecting member.
    #[must_use]
    pub fn open_relay(&self, doc: DocumentId, actor: UserId, can_write: bool) -> DocRelay {
        DocRelay {
            state: self.clone(),
            doc,
            actor,
            can_write,
            origin: Origin {
                instance: self.instance,
                conn: self.next_conn.fetch_add(1, Ordering::Relaxed),
            },
        }
    }
}

/// One member's live connection to one document.
pub struct DocRelay {
    state: CollabState,
    doc: DocumentId,
    actor: UserId,
    can_write: bool,
    origin: Origin,
}

impl DocRelay {
    /// The connection's bus origin (used to skip its own echoes).
    #[must_use]
    pub fn origin(&self) -> Origin {
        self.origin
    }

    /// Subscribes to the document's event stream.
    pub async fn subscribe(&self) -> tokio::sync::broadcast::Receiver<BusEvent> {
        self.state.bus.subscribe(self.doc).await
    }

    /// Replays persisted updates with `seq > after` (connection catch-up:
    /// the client loads the snapshot via REST, then resumes from its seq).
    pub async fn backlog(&self, after: i64) -> Result<Vec<CollabFrame>, RelayError> {
        let entries = self.state.oplog.fetch_after(self.doc, after).await?;
        Ok(entries
            .into_iter()
            .map(|e| CollabFrame::Update(e.payload))
            .collect())
    }

    /// Handles one frame received from this connection's client.
    ///
    /// Updates are appended to the oplog first, then fanned out — a crash
    /// between the two loses no data (subscribers resync from the oplog).
    pub async fn ingest(&self, frame: CollabFrame) -> Result<(), RelayError> {
        match frame {
            CollabFrame::Update(payload) => {
                if !self.can_write {
                    return Err(RelayError::WriteDenied);
                }
                let seq = self
                    .state
                    .oplog
                    .append(self.doc, self.actor, &payload)
                    .await?;
                self.state
                    .bus
                    .publish_update(self.origin, self.doc, seq, &payload)
                    .await?;
            }
            CollabFrame::Awareness(payload) => {
                // Awareness is broadcast-only and never persisted (ADR-C013);
                // read-only members may still share their cursor.
                self.state
                    .bus
                    .publish_awareness(self.origin, self.doc, &payload)
                    .await?;
            }
        }
        Ok(())
    }

    /// Whether an event from the bus should be forwarded to this client.
    #[must_use]
    pub fn wants(&self, event: &BusEvent) -> bool {
        event.origin != self.origin
    }
}

/// Relay failures surfaced to the WebSocket adapter.
#[derive(Debug, thiserror::Error)]
pub enum RelayError {
    /// The member's role does not allow content writes (ADR-C017).
    #[error("member role does not permit content writes")]
    WriteDenied,
    /// Persistence failed.
    #[error(transparent)]
    Store(#[from] StoreError),
    /// Fan-out failed.
    #[error(transparent)]
    Bus(#[from] BusError),
}

#[cfg(test)]
#[path = "relay_tests.rs"]
mod tests;
