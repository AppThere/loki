// SPDX-License-Identifier: Apache-2.0

//! Postgres `LISTEN`/`NOTIFY` fan-out — the zero-extra-infra default
//! (ADR-C012, ratified decision §6.2).
//!
//! `NOTIFY` payloads are capped (~8 kB), so updates travel as *pointers*
//! (`doc_id` + oplog `seq`) and are re-read from the oplog on the receiving
//! instance — the update is already durable before publication (ADR-C013).
//! Awareness is ephemeral and small (cursors), so it is inlined base64;
//! oversized awareness is a typed error, not a silent drop.

use std::sync::Arc;

use async_trait::async_trait;
use base64::Engine as _;
use base64::engine::general_purpose::STANDARD as BASE64;
use loki_model::DocumentId;
use loki_server_store::OplogStore;
use serde::{Deserialize, Serialize};
use sqlx::postgres::{PgListener, PgPool};
use tokio::sync::broadcast;
use uuid::Uuid;

use crate::bus::{BusError, BusEvent, FanOutBus, Origin};
use crate::hub::LocalHub;
use crate::msg::CollabFrame;

/// The Postgres notification channel.
const CHANNEL: &str = "loki_collab";

/// Maximum raw awareness payload fanned out cross-instance. Base64 expansion
/// (×4/3) plus the JSON envelope must stay under Postgres' ~8000-byte
/// `NOTIFY` limit.
const MAX_AWARENESS_BYTES: usize = 4096;

#[derive(Serialize, Deserialize)]
struct Notification {
    instance: Uuid,
    conn: u64,
    doc: Uuid,
    #[serde(flatten)]
    body: Body,
}

#[derive(Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "kebab-case")]
enum Body {
    Update { seq: i64 },
    Awareness { data: String },
}

/// `LISTEN`/`NOTIFY`-backed fan-out bus.
///
/// The `instance` id passed to [`PgNotifyBus::start`] filters out this
/// process' own `NOTIFY` echoes in the fan-in task; publishers stamp the
/// same id into [`Origin::instance`].
pub struct PgNotifyBus {
    pool: PgPool,
    hub: Arc<LocalHub>,
}

impl PgNotifyBus {
    /// Connects the listener and starts the background fan-in task.
    ///
    /// `oplog` is used to re-read update payloads notified by other
    /// instances; `instance` must match the [`Origin::instance`] used by
    /// local publishers so self-notifications are skipped.
    pub async fn start(
        pool: PgPool,
        oplog: Arc<dyn OplogStore>,
        instance: Uuid,
    ) -> Result<Arc<Self>, BusError> {
        let mut listener = PgListener::connect_with(&pool).await?;
        listener.listen(CHANNEL).await?;
        let bus = Arc::new(Self {
            pool,
            hub: Arc::new(LocalHub::new()),
        });
        let hub = Arc::clone(&bus.hub);
        tokio::spawn(async move {
            fan_in_loop(listener, hub, oplog, instance).await;
        });
        Ok(bus)
    }

    async fn notify(&self, notification: &Notification) -> Result<(), BusError> {
        let payload = serde_json::to_string(notification)
            .map_err(|e| BusError::MalformedNotification(e.to_string()))?;
        sqlx::query("SELECT pg_notify($1, $2)")
            .bind(CHANNEL)
            .bind(payload)
            .execute(&self.pool)
            .await?;
        Ok(())
    }
}

/// Receives notifications from other instances and re-publishes them locally.
async fn fan_in_loop(
    mut listener: PgListener,
    hub: Arc<LocalHub>,
    oplog: Arc<dyn OplogStore>,
    instance: Uuid,
) {
    loop {
        let notification = match listener.recv().await {
            Ok(n) => n,
            Err(error) => {
                // PgListener reconnects internally; a returned error is a
                // failed reconnect attempt. Keep trying — collaboration
                // degrades to single-instance until Postgres is back.
                tracing::warn!(%error, "collab bus listener error; retrying");
                continue;
            }
        };
        if let Some(event) = event_from_payload(notification.payload(), &*oplog, instance).await {
            hub.publish(event);
        }
    }
}

/// Turns one raw `NOTIFY` payload into the local event it should produce.
///
/// `None` means "deliver nothing", which is the correct outcome for every
/// tolerated condition: our own echo (already delivered locally), a malformed
/// envelope, an update compacted before we could re-read it (subscribers
/// resync from the snapshot — ADR-C013 recovery path), an oplog read failure,
/// and an undecodable awareness blob. None of these may kill the fan-in task.
///
/// Split out of [`fan_in_loop`] so the envelope handling is testable without
/// a Postgres listener.
async fn event_from_payload(
    payload: &str,
    oplog: &dyn OplogStore,
    instance: Uuid,
) -> Option<BusEvent> {
    let parsed: Notification = match serde_json::from_str(payload) {
        Ok(p) => p,
        Err(error) => {
            tracing::warn!(%error, "ignoring malformed collab notification");
            return None;
        }
    };
    if parsed.instance == instance {
        return None; // Our own NOTIFY echo; already delivered locally.
    }
    let doc = DocumentId::from_uuid(parsed.doc);
    let origin = Origin {
        instance: parsed.instance,
        conn: parsed.conn,
    };
    let frame = match parsed.body {
        Body::Update { seq } => match oplog.fetch_one(doc, seq).await {
            Ok(Some(entry)) => CollabFrame::Update(entry.payload),
            Ok(None) => {
                tracing::debug!(%doc, seq, "notified update already compacted");
                return None;
            }
            Err(error) => {
                tracing::warn!(%error, %doc, seq, "oplog fetch for notified update failed");
                return None;
            }
        },
        Body::Awareness { data } => match BASE64.decode(data.as_bytes()) {
            Ok(bytes) => CollabFrame::Awareness(bytes),
            Err(error) => {
                tracing::warn!(%error, "ignoring undecodable awareness payload");
                return None;
            }
        },
    };
    Some(BusEvent { doc, origin, frame })
}

#[async_trait]
impl FanOutBus for PgNotifyBus {
    async fn publish_update(
        &self,
        origin: Origin,
        doc: DocumentId,
        seq: i64,
        payload: &[u8],
    ) -> Result<(), BusError> {
        // Local subscribers get the payload directly …
        self.hub.publish(BusEvent {
            doc,
            origin,
            frame: CollabFrame::Update(payload.to_vec()),
        });
        // … other instances get a pointer and re-read the oplog.
        self.notify(&Notification {
            instance: origin.instance,
            conn: origin.conn,
            doc: doc.as_uuid(),
            body: Body::Update { seq },
        })
        .await
    }

    async fn publish_awareness(
        &self,
        origin: Origin,
        doc: DocumentId,
        payload: &[u8],
    ) -> Result<(), BusError> {
        if payload.len() > MAX_AWARENESS_BYTES {
            return Err(BusError::AwarenessTooLarge(payload.len()));
        }
        self.hub.publish(BusEvent {
            doc,
            origin,
            frame: CollabFrame::Awareness(payload.to_vec()),
        });
        self.notify(&Notification {
            instance: origin.instance,
            conn: origin.conn,
            doc: doc.as_uuid(),
            body: Body::Awareness {
                data: BASE64.encode(payload),
            },
        })
        .await
    }

    async fn subscribe(&self, doc: DocumentId) -> broadcast::Receiver<BusEvent> {
        self.hub.subscribe(doc)
    }
}

#[cfg(test)]
#[path = "bus_pg_tests.rs"]
mod tests;
