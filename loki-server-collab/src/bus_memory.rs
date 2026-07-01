// SPDX-License-Identifier: Apache-2.0

//! Single-process fan-out (tests, embedded/local use).

use async_trait::async_trait;
use loki_model::DocumentId;
use tokio::sync::broadcast;

use crate::bus::{BusError, BusEvent, FanOutBus, Origin};
use crate::hub::LocalHub;
use crate::msg::CollabFrame;

/// In-process bus: every subscriber lives in this process, so fan-out is
/// just the local hub.
#[derive(Default)]
pub struct InMemoryBus {
    hub: LocalHub,
}

impl InMemoryBus {
    /// Creates an empty bus.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }
}

#[async_trait]
impl FanOutBus for InMemoryBus {
    async fn publish_update(
        &self,
        origin: Origin,
        doc: DocumentId,
        _seq: i64,
        payload: &[u8],
    ) -> Result<(), BusError> {
        self.hub.publish(BusEvent {
            doc,
            origin,
            frame: CollabFrame::Update(payload.to_vec()),
        });
        Ok(())
    }

    async fn publish_awareness(
        &self,
        origin: Origin,
        doc: DocumentId,
        payload: &[u8],
    ) -> Result<(), BusError> {
        self.hub.publish(BusEvent {
            doc,
            origin,
            frame: CollabFrame::Awareness(payload.to_vec()),
        });
        Ok(())
    }

    async fn subscribe(&self, doc: DocumentId) -> broadcast::Receiver<BusEvent> {
        self.hub.subscribe(doc)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use uuid::Uuid;

    #[tokio::test]
    async fn update_reaches_all_subscribers() {
        let bus = InMemoryBus::new();
        let doc = DocumentId::new();
        let mut rx1 = bus.subscribe(doc).await;
        let mut rx2 = bus.subscribe(doc).await;
        let origin = Origin {
            instance: Uuid::nil(),
            conn: 7,
        };
        bus.publish_update(origin, doc, 1, b"update").await.unwrap();
        for rx in [&mut rx1, &mut rx2] {
            let event = rx.recv().await.unwrap();
            assert_eq!(event.frame, CollabFrame::Update(b"update".to_vec()));
            assert_eq!(event.origin, origin);
        }
    }
}
