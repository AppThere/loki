// SPDX-License-Identifier: Apache-2.0

//! Per-document local broadcast channels, shared by all bus implementations.

use std::collections::HashMap;
use std::sync::Mutex;

use loki_model::DocumentId;
use tokio::sync::broadcast;

use crate::bus::BusEvent;

/// Buffered events per document channel. A slow consumer that falls more
/// than this many events behind observes a `Lagged` error and must resync
/// from the oplog (the updates themselves are never lost — they are durable
/// before publication).
const CHANNEL_CAPACITY: usize = 256;

/// Local (in-process) per-document broadcast hub.
#[derive(Default)]
pub(crate) struct LocalHub {
    channels: Mutex<HashMap<DocumentId, broadcast::Sender<BusEvent>>>,
}

impl LocalHub {
    pub(crate) fn new() -> Self {
        Self::default()
    }

    fn lock(&self) -> std::sync::MutexGuard<'_, HashMap<DocumentId, broadcast::Sender<BusEvent>>> {
        match self.channels.lock() {
            Ok(guard) => guard,
            Err(poisoned) => poisoned.into_inner(),
        }
    }

    /// Subscribes to a document, creating its channel on first use.
    pub(crate) fn subscribe(&self, doc: DocumentId) -> broadcast::Receiver<BusEvent> {
        let mut channels = self.lock();
        channels
            .entry(doc)
            .or_insert_with(|| broadcast::channel(CHANNEL_CAPACITY).0)
            .subscribe()
    }

    /// Delivers an event to local subscribers (no-op when nobody listens).
    pub(crate) fn publish(&self, event: BusEvent) {
        let mut channels = self.lock();
        if let Some(sender) = channels.get(&event.doc) {
            if sender.send(event.clone()).is_err() {
                // Last receiver dropped: reclaim the channel.
                channels.remove(&event.doc);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::bus::Origin;
    use crate::msg::CollabFrame;
    use uuid::Uuid;

    fn event(doc: DocumentId) -> BusEvent {
        BusEvent {
            doc,
            origin: Origin {
                instance: Uuid::nil(),
                conn: 1,
            },
            frame: CollabFrame::Update(b"u".to_vec()),
        }
    }

    #[tokio::test]
    async fn subscribers_only_see_their_document() {
        let hub = LocalHub::new();
        let doc_a = DocumentId::new();
        let doc_b = DocumentId::new();
        let mut rx_a = hub.subscribe(doc_a);
        let mut rx_b = hub.subscribe(doc_b);

        hub.publish(event(doc_a));
        assert_eq!(rx_a.recv().await.unwrap().doc, doc_a);
        assert!(rx_b.try_recv().is_err());
    }

    #[tokio::test]
    async fn dropped_channel_is_reclaimed() {
        let hub = LocalHub::new();
        let doc = DocumentId::new();
        drop(hub.subscribe(doc));
        hub.publish(event(doc)); // send fails → entry removed
        assert!(hub.lock().is_empty());
    }
}
