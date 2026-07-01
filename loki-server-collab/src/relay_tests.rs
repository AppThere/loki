// SPDX-License-Identifier: Apache-2.0

//! Relay tests against the in-memory oplog and bus.

use loki_server_store::memory::MemoryStores;
use loki_server_store::OplogStore as _;

use crate::bus_memory::InMemoryBus;

use super::*;

fn test_state() -> (CollabState, Arc<MemoryStores>) {
    let stores = Arc::new(MemoryStores::new());
    let state = CollabState::new(
        Arc::clone(&stores) as Arc<dyn OplogStore>,
        Arc::new(InMemoryBus::new()),
        Uuid::new_v4(),
    );
    (state, stores)
}

#[tokio::test]
async fn update_is_persisted_then_broadcast() {
    let (state, stores) = test_state();
    let doc = DocumentId::new();
    let alice = state.open_relay(doc, UserId::new(), true);
    let bob = state.open_relay(doc, UserId::new(), false);
    let mut bob_rx = bob.subscribe().await;

    alice.ingest(CollabFrame::Update(b"op-1".to_vec())).await.unwrap();

    // Durable first …
    let entries = stores.fetch_after(doc, 0).await.unwrap();
    assert_eq!(entries.len(), 1);
    assert_eq!(entries[0].payload, b"op-1");
    // … then delivered.
    let event = bob_rx.recv().await.unwrap();
    assert_eq!(event.frame, CollabFrame::Update(b"op-1".to_vec()));
    assert!(bob.wants(&event));
    assert!(!alice.wants(&event), "sender skips its own echo");
}

#[tokio::test]
async fn read_only_member_cannot_write_but_may_share_awareness() {
    let (state, stores) = test_state();
    let doc = DocumentId::new();
    let viewer = state.open_relay(doc, UserId::new(), false);

    let denied = viewer.ingest(CollabFrame::Update(b"op".to_vec())).await;
    assert!(matches!(denied, Err(RelayError::WriteDenied)));
    assert!(stores.fetch_after(doc, 0).await.unwrap().is_empty());

    viewer
        .ingest(CollabFrame::Awareness(b"cursor@3".to_vec()))
        .await
        .unwrap();
    // Awareness is never persisted (ADR-C013).
    assert!(stores.fetch_after(doc, 0).await.unwrap().is_empty());
}

#[tokio::test]
async fn backlog_replays_updates_after_seq() {
    let (state, _stores) = test_state();
    let doc = DocumentId::new();
    let writer = state.open_relay(doc, UserId::new(), true);
    for payload in [b"a".as_slice(), b"b", b"c"] {
        writer.ingest(CollabFrame::Update(payload.to_vec())).await.unwrap();
    }
    let all = writer.backlog(0).await.unwrap();
    assert_eq!(all.len(), 3);
    let tail = writer.backlog(2).await.unwrap();
    assert_eq!(tail, vec![CollabFrame::Update(b"c".to_vec())]);
}

#[tokio::test]
async fn events_do_not_cross_documents() {
    let (state, _stores) = test_state();
    let doc_a = DocumentId::new();
    let doc_b = DocumentId::new();
    let writer_a = state.open_relay(doc_a, UserId::new(), true);
    let reader_b = state.open_relay(doc_b, UserId::new(), false);
    let mut rx_b = reader_b.subscribe().await;

    writer_a.ingest(CollabFrame::Update(b"op".to_vec())).await.unwrap();
    assert!(rx_b.try_recv().is_err());
}
