// SPDX-License-Identifier: Apache-2.0

//! Compaction tests with real Loro documents over the in-memory ports.

use chrono::Utc;
use loki_model::{EncryptionTier, Residency, UserId, WorkspaceId};
use loki_server_store::memory::MemoryStores;
use loki_server_store::{DocMetaRecord, OplogStore as _};
use object_store::memory::InMemory;

use super::*;

struct Fixture {
    compactor: Compactor,
    stores: Arc<MemoryStores>,
    blob: BlobStore,
}

fn fixture() -> Fixture {
    let stores = Arc::new(MemoryStores::new());
    let blob = BlobStore::new(Arc::new(InMemory::new()));
    let compactor = Compactor::new(
        Arc::clone(&stores) as Arc<dyn DocumentStore>,
        Arc::clone(&stores) as Arc<dyn OplogStore>,
        blob.clone(),
    );
    Fixture {
        compactor,
        stores,
        blob,
    }
}

async fn create_doc(stores: &MemoryStores, tier: EncryptionTier) -> DocumentId {
    let doc = DocMetaRecord {
        id: DocumentId::new(),
        workspace_id: WorkspaceId::new(),
        title: "doc".into(),
        tier,
        residency: Residency::parse("fsn1").unwrap(),
        snapshot_ptr: None,
        snapshot_seq: 0,
        dek_wrapped: None,
        created_at: Utc::now(),
    };
    stores.create_document(&doc).await.unwrap();
    doc.id
}

/// Produces a Loro update appending `text` to the shared text container.
fn loro_update(peer: u64, base: Option<&[u8]>, text: &str) -> Vec<u8> {
    let doc = LoroDoc::new();
    doc.set_peer_id(peer).unwrap();
    if let Some(base) = base {
        doc.import(base).unwrap();
    }
    let before = doc.oplog_vv();
    let t = doc.get_text("t");
    let end = t.len_unicode();
    t.insert(end, text).unwrap();
    doc.export(loro::ExportMode::updates(&before)).unwrap()
}

fn text_of(snapshot: &[u8]) -> String {
    let doc = LoroDoc::new();
    doc.import(snapshot).unwrap();
    doc.get_text("t").to_string()
}

use loki_server_store::DocumentStore;

#[tokio::test]
async fn compacts_oplog_into_snapshot_and_truncates() {
    let f = fixture();
    let doc = create_doc(&f.stores, EncryptionTier::TransportAtRest).await;
    let actor = UserId::new();

    // Three sequential updates building "abc".
    let u1 = loro_update(1, None, "a");
    let u2 = loro_update(1, Some(&u1), "b");
    let merged = {
        let d = LoroDoc::new();
        d.import(&u1).unwrap();
        d.import(&u2).unwrap();
        d.export(loro::ExportMode::Snapshot).unwrap()
    };
    let u3 = loro_update(1, Some(&merged), "c");
    for update in [&u1, &u2, &u3] {
        f.stores.append(doc, actor, update).await.unwrap();
    }

    let outcome = f.compactor.compact_document(doc).await.unwrap();
    assert_eq!(
        outcome,
        CompactionOutcome::Compacted {
            up_to_seq: 3,
            entries: 3
        }
    );

    // Snapshot content is the merged document; oplog is empty; pointer set.
    let meta = f.stores.get_document(doc).await.unwrap().unwrap();
    assert_eq!(meta.snapshot_seq, 3);
    let snapshot = f
        .blob
        .get(meta.snapshot_ptr.as_deref().unwrap())
        .await
        .unwrap();
    assert_eq!(text_of(&snapshot), "abc");
    assert!(f.stores.fetch_after(doc, 0).await.unwrap().is_empty());

    // Nothing new → NothingToDo.
    assert_eq!(
        f.compactor.compact_document(doc).await.unwrap(),
        CompactionOutcome::NothingToDo
    );
}

#[tokio::test]
async fn second_pass_builds_on_previous_snapshot() {
    let f = fixture();
    let doc = create_doc(&f.stores, EncryptionTier::CustomerManagedKeys).await;
    let actor = UserId::new();

    let u1 = loro_update(1, None, "hello");
    f.stores.append(doc, actor, &u1).await.unwrap();
    f.compactor.compact_document(doc).await.unwrap();

    // A later update on top of the compacted state.
    let meta = f.stores.get_document(doc).await.unwrap().unwrap();
    let base = f
        .blob
        .get(meta.snapshot_ptr.as_deref().unwrap())
        .await
        .unwrap();
    let u2 = loro_update(1, Some(&base), " world");
    f.stores.append(doc, actor, &u2).await.unwrap();

    let outcome = f.compactor.compact_document(doc).await.unwrap();
    assert!(matches!(
        outcome,
        CompactionOutcome::Compacted { entries: 1, .. }
    ));
    let meta = f.stores.get_document(doc).await.unwrap().unwrap();
    let snapshot = f
        .blob
        .get(meta.snapshot_ptr.as_deref().unwrap())
        .await
        .unwrap();
    assert_eq!(text_of(&snapshot), "hello world");
}

#[tokio::test]
async fn zero_knowledge_documents_are_never_compacted() {
    let f = fixture();
    let doc = create_doc(&f.stores, EncryptionTier::ZeroKnowledge).await;
    f.stores
        .append(doc, UserId::new(), b"ciphertext-not-loro")
        .await
        .unwrap();

    assert_eq!(
        f.compactor.compact_document(doc).await.unwrap(),
        CompactionOutcome::SkippedZeroKnowledge
    );
    // The ciphertext stays in the oplog untouched.
    assert_eq!(f.stores.fetch_after(doc, 0).await.unwrap().len(), 1);
}

#[tokio::test]
async fn corrupt_payload_aborts_without_truncating() {
    let f = fixture();
    let doc = create_doc(&f.stores, EncryptionTier::TransportAtRest).await;
    f.stores
        .append(doc, UserId::new(), b"not a loro update")
        .await
        .unwrap();

    let result = f.compactor.compact_document(doc).await;
    assert!(matches!(result, Err(CompactError::Loro(_))));
    // Nothing was truncated and no pointer moved.
    assert_eq!(f.stores.fetch_after(doc, 0).await.unwrap().len(), 1);
    let meta = f.stores.get_document(doc).await.unwrap().unwrap();
    assert_eq!(meta.snapshot_seq, 0);
    assert!(meta.snapshot_ptr.is_none());
}

#[tokio::test]
async fn lost_race_leaves_newer_snapshot_intact() {
    let f = fixture();
    let doc = create_doc(&f.stores, EncryptionTier::TransportAtRest).await;
    let actor = UserId::new();
    f.stores
        .append(doc, actor, &loro_update(1, None, "x"))
        .await
        .unwrap();

    // Simulate a faster compactor having already covered seq 5.
    assert!(f.stores.set_snapshot(doc, "winner-ptr", 5).await.unwrap());

    let outcome = f.compactor.compact_document(doc).await.unwrap();
    // Entry seq 1 <= snapshot_seq 5, so there is no tail to compact.
    assert_eq!(outcome, CompactionOutcome::NothingToDo);
    let meta = f.stores.get_document(doc).await.unwrap().unwrap();
    assert_eq!(meta.snapshot_ptr.as_deref(), Some("winner-ptr"));
    assert_eq!(meta.snapshot_seq, 5);
}
