// SPDX-License-Identifier: Apache-2.0

//! Compaction tests with real Loro documents over the in-memory ports.

use std::sync::atomic::{AtomicBool, Ordering};

use async_trait::async_trait;
use chrono::Utc;
use futures_util::StreamExt as _;
use loki_model::{EncryptionTier, Residency, UserId, WorkspaceId};
use loki_server_store::memory::MemoryStores;
use loki_server_store::{DocMetaRecord, OplogEntry, OplogStore as _, StoreError};
use object_store::ObjectStore;
use object_store::memory::InMemory;

use super::*;

struct Fixture {
    compactor: Compactor,
    stores: Arc<MemoryStores>,
    blob: BlobStore,
    objects: Arc<InMemory>,
}

fn fixture() -> Fixture {
    fixture_with(|stores| Arc::clone(stores) as Arc<dyn OplogStore>)
}

/// Builds a fixture whose compactor reads the oplog through `oplog` — a hook
/// so a test can interpose on the port without touching production code.
fn fixture_with(oplog: impl FnOnce(&Arc<MemoryStores>) -> Arc<dyn OplogStore>) -> Fixture {
    let stores = Arc::new(MemoryStores::new());
    let objects = Arc::new(InMemory::new());
    let blob = BlobStore::new(Arc::clone(&objects) as Arc<dyn ObjectStore>);
    let compactor = Compactor::new(
        Arc::clone(&stores) as Arc<dyn DocumentStore>,
        oplog(&stores),
        blob.clone(),
    );
    Fixture {
        compactor,
        stores,
        blob,
        objects,
    }
}

/// Every key currently present in the blob store.
async fn blob_keys(objects: &InMemory) -> Vec<String> {
    objects
        .list(None)
        .map(|meta| meta.unwrap().location.to_string())
        .collect::<Vec<_>>()
        .await
}

/// An [`OplogStore`] that delegates to [`MemoryStores`] but, on the **first**
/// `fetch_after`, lets a simulated faster compactor advance the snapshot
/// pointer — i.e. it interleaves the race exactly where ADR-C013's guard has
/// to hold: the tail has been read against a `snapshot_seq` that is stale by
/// the time `set_snapshot` is called.
///
/// With `winner: None` it is a pass-through, which is the control: it proves
/// the wrapper itself does not suppress compaction.
struct RacingOplog {
    inner: Arc<MemoryStores>,
    winner: Option<(String, i64)>,
    fired: AtomicBool,
}

impl RacingOplog {
    fn new(inner: &Arc<MemoryStores>, winner: Option<(String, i64)>) -> Arc<Self> {
        Arc::new(Self {
            inner: Arc::clone(inner),
            winner,
            fired: AtomicBool::new(false),
        })
    }
}

#[async_trait]
impl OplogStore for RacingOplog {
    async fn append(
        &self,
        doc: DocumentId,
        actor: UserId,
        payload: &[u8],
    ) -> Result<i64, StoreError> {
        self.inner.append(doc, actor, payload).await
    }

    async fn fetch_after(
        &self,
        doc: DocumentId,
        after: i64,
    ) -> Result<Vec<OplogEntry>, StoreError> {
        let entries = self.inner.fetch_after(doc, after).await?;
        if let Some((ptr, up_to)) = &self.winner
            && !self.fired.swap(true, Ordering::SeqCst)
        {
            assert!(
                self.inner.set_snapshot(doc, ptr, *up_to).await?,
                "the simulated winner must actually win the pointer guard"
            );
        }
        Ok(entries)
    }

    async fn fetch_one(&self, doc: DocumentId, seq: i64) -> Result<Option<OplogEntry>, StoreError> {
        self.inner.fetch_one(doc, seq).await
    }

    async fn truncate_up_to(&self, doc: DocumentId, up_to: i64) -> Result<(), StoreError> {
        self.inner.truncate_up_to(doc, up_to).await
    }

    async fn docs_with_backlog(
        &self,
        min_entries: i64,
    ) -> Result<Vec<(DocumentId, i64)>, StoreError> {
        self.inner.docs_with_backlog(min_entries).await
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

/// Pins the *early-exit* path: when the pointer is already ahead of the whole
/// oplog tail before the pass starts, `fetch_after` returns nothing and the
/// pass ends at `NothingToDo` — it never reaches the `set_snapshot` guard.
/// (Formerly named `lost_race_leaves_newer_snapshot_intact`, which it is not:
/// the `LostRace` branch is covered by `lost_race_*` below.)
#[tokio::test]
async fn snapshot_ahead_of_the_tail_is_nothing_to_do() {
    let f = fixture();
    let doc = create_doc(&f.stores, EncryptionTier::TransportAtRest).await;
    let actor = UserId::new();
    f.stores
        .append(doc, actor, &loro_update(1, None, "x"))
        .await
        .unwrap();

    // A faster compactor already covered seq 5 *before* this pass starts.
    assert!(f.stores.set_snapshot(doc, "winner-ptr", 5).await.unwrap());

    let outcome = f.compactor.compact_document(doc).await.unwrap();
    // Entry seq 1 <= snapshot_seq 5, so there is no tail to compact.
    assert_eq!(outcome, CompactionOutcome::NothingToDo);
    let meta = f.stores.get_document(doc).await.unwrap().unwrap();
    assert_eq!(meta.snapshot_ptr.as_deref(), Some("winner-ptr"));
    assert_eq!(meta.snapshot_seq, 5);
    // Nothing was written, so nothing needed deleting.
    assert!(blob_keys(&f.objects).await.is_empty());
}

/// Control for the race test below: the same interposing wrapper, with the
/// interleaved write disabled, must still compact normally. Without this the
/// `LostRace` assertion could be satisfied by a wrapper that merely breaks the
/// pass (CLAUDE.md evidence rule 3: establish the phenomenon is reachable
/// *without* the control before using the control to reveal it).
#[tokio::test]
async fn interposing_oplog_wrapper_still_compacts_when_uncontested() {
    let f = fixture_with(|stores| RacingOplog::new(stores, None) as Arc<dyn OplogStore>);
    let doc = create_doc(&f.stores, EncryptionTier::TransportAtRest).await;
    f.stores
        .append(doc, UserId::new(), &loro_update(1, None, "x"))
        .await
        .unwrap();

    assert_eq!(
        f.compactor.compact_document(doc).await.unwrap(),
        CompactionOutcome::Compacted {
            up_to_seq: 1,
            entries: 1
        }
    );
    // The winner's own blob survives, and the covered entry is truncated.
    assert_eq!(
        blob_keys(&f.objects).await,
        vec![BlobStore::snapshot_key(doc, 1)]
    );
    assert!(f.stores.fetch_after(doc, 0).await.unwrap().is_empty());
}

/// The real ADR-C013 race (F-CO-1): a concurrent compactor advances the
/// pointer *between* this pass' `fetch_after` and its `set_snapshot`. The
/// guard must reject the stale pointer write, and the loser must clean up
/// after itself **without truncating** — an orphaned truncation would drop
/// updates the winner's snapshot does not cover.
#[tokio::test]
async fn lost_race_deletes_the_loser_blob_and_never_truncates() {
    let f = fixture_with(|stores| {
        RacingOplog::new(stores, Some(("winner-ptr".to_owned(), 5))) as Arc<dyn OplogStore>
    });
    let doc = create_doc(&f.stores, EncryptionTier::TransportAtRest).await;
    let update = loro_update(1, None, "x");
    f.stores.append(doc, UserId::new(), &update).await.unwrap();

    let outcome = f.compactor.compact_document(doc).await.unwrap();

    // 1. The guard was reached and refused the regressing pointer.
    assert_eq!(outcome, CompactionOutcome::LostRace);
    // 2. The loser's freshly-written snapshot blob is gone — no orphan.
    assert!(
        f.blob.get(&BlobStore::snapshot_key(doc, 1)).await.is_err(),
        "the loser's snapshot blob must be deleted"
    );
    assert!(
        blob_keys(&f.objects).await.is_empty(),
        "no orphaned object may remain in the blob store"
    );
    // 3. The oplog was NOT truncated: the winner's snapshot covers seq 5, but
    //    this entry's payload must survive regardless — losing the guard must
    //    never cost data.
    let remaining = f.stores.fetch_after(doc, 0).await.unwrap();
    assert_eq!(remaining.len(), 1, "the oplog must not be truncated");
    assert_eq!(remaining[0].payload, update);
    // And the winner's pointer is untouched.
    let meta = f.stores.get_document(doc).await.unwrap().unwrap();
    assert_eq!(meta.snapshot_ptr.as_deref(), Some("winner-ptr"));
    assert_eq!(meta.snapshot_seq, 5);
}
