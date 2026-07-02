// SPDX-License-Identifier: Apache-2.0

//! Server-side snapshot compaction (ADR-C013).
//!
//! For Tier-0/1 documents the server periodically merges the current
//! snapshot with the oplog tail into a fresh Loro snapshot, stores it, and
//! truncates the covered oplog entries. This is the *only* place the server
//! interprets CRDT bytes — and only where the tier permits it: Tier-2
//! documents are skipped entirely (the server holds ciphertext; clients
//! upload their own encrypted snapshots via `PUT …/snapshot`).
//!
//! Safety order: snapshot write → pointer advance (guarded, forward-only)
//! → oplog truncation. A crash between steps leaves extra oplog entries
//! (harmless — Loro imports are idempotent), never missing ones. The
//! forward-only pointer guard in the store makes concurrent compactors
//! race-safe: the loser skips truncation.

use std::sync::Arc;
use std::time::Duration;

use loki_model::DocumentId;
use loki_server_store::{BlobStore, DocumentStore, OplogStore, StoreError};
use loro::LoroDoc;

/// Compacts document oplogs into snapshots.
pub struct Compactor {
    documents: Arc<dyn DocumentStore>,
    oplog: Arc<dyn OplogStore>,
    blob: BlobStore,
}

/// What a compaction pass did for one document.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CompactionOutcome {
    /// A new snapshot now covers everything up to `up_to_seq`.
    Compacted {
        /// Highest oplog sequence included in the snapshot.
        up_to_seq: i64,
        /// Number of oplog entries folded in (and truncated).
        entries: usize,
    },
    /// The oplog held nothing newer than the current snapshot.
    NothingToDo,
    /// Tier-2 document: the server never compacts ciphertext (ADR-C014).
    SkippedZeroKnowledge,
    /// A concurrent compactor advanced the snapshot first; nothing changed.
    LostRace,
}

/// Compaction failures. The oplog is only ever truncated after a snapshot
/// covering it is durably stored and the pointer has advanced.
#[derive(Debug, thiserror::Error)]
pub enum CompactError {
    /// The document disappeared (deleted mid-pass).
    #[error("document not found")]
    DocumentNotFound,
    /// Persistence failed.
    #[error(transparent)]
    Store(#[from] StoreError),
    /// The stored snapshot or an oplog payload was not importable. This is
    /// data corruption (or a non-Loro payload on a Tier-0/1 document) — the
    /// pass aborts without truncating anything.
    #[error("loro import/export failed: {0}")]
    Loro(String),
}

impl Compactor {
    /// Creates a compactor over the given ports.
    #[must_use]
    pub fn new(
        documents: Arc<dyn DocumentStore>,
        oplog: Arc<dyn OplogStore>,
        blob: BlobStore,
    ) -> Self {
        Self {
            documents,
            oplog,
            blob,
        }
    }

    /// Compacts one document, if its tier allows and there is a backlog.
    pub async fn compact_document(
        &self,
        doc: DocumentId,
    ) -> Result<CompactionOutcome, CompactError> {
        let meta = self
            .documents
            .get_document(doc)
            .await?
            .ok_or(CompactError::DocumentNotFound)?;
        if !meta.tier.server_compacts_snapshots() {
            return Ok(CompactionOutcome::SkippedZeroKnowledge);
        }

        let entries = self.oplog.fetch_after(doc, meta.snapshot_seq).await?;
        let Some(last) = entries.last() else {
            return Ok(CompactionOutcome::NothingToDo);
        };
        let up_to_seq = last.seq;
        let count = entries.len();

        // Rebuild the document: current snapshot (if any) + tail.
        let loro = LoroDoc::new();
        if let Some(ptr) = &meta.snapshot_ptr {
            let snapshot = self.blob.get(ptr).await?;
            loro.import(&snapshot)
                .map_err(|e| CompactError::Loro(e.to_string()))?;
        }
        for entry in &entries {
            loro.import(&entry.payload)
                .map_err(|e| CompactError::Loro(e.to_string()))?;
        }
        let snapshot = loro
            .export(loro::ExportMode::Snapshot)
            .map_err(|e| CompactError::Loro(e.to_string()))?;

        // Durable snapshot first; only the guard winner truncates.
        let ptr = self.blob.put_snapshot(doc, up_to_seq, snapshot).await?;
        if !self.documents.set_snapshot(doc, &ptr, up_to_seq).await? {
            self.blob.delete(&ptr).await?;
            return Ok(CompactionOutcome::LostRace);
        }
        self.oplog.truncate_up_to(doc, up_to_seq).await?;
        Ok(CompactionOutcome::Compacted {
            up_to_seq,
            entries: count,
        })
    }

    /// Runs forever: every `interval`, compacts each document whose oplog
    /// holds at least `min_entries` updates. Per-document failures are
    /// logged and skipped — one corrupt document must not stall the rest.
    pub async fn run_periodic(self: Arc<Self>, interval: Duration, min_entries: i64) {
        let mut ticker = tokio::time::interval(interval);
        ticker.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Delay);
        loop {
            ticker.tick().await;
            let candidates = match self.oplog.docs_with_backlog(min_entries).await {
                Ok(candidates) => candidates,
                Err(error) => {
                    tracing::warn!(%error, "compaction candidate scan failed");
                    continue;
                }
            };
            for (doc, backlog) in candidates {
                match self.compact_document(doc).await {
                    Ok(CompactionOutcome::Compacted { up_to_seq, entries }) => {
                        tracing::info!(%doc, up_to_seq, entries, "compacted oplog into snapshot");
                    }
                    Ok(_) => {}
                    Err(error) => {
                        tracing::warn!(%error, %doc, backlog, "compaction failed; will retry");
                    }
                }
            }
        }
    }
}

#[cfg(test)]
#[path = "compact_tests.rs"]
mod tests;
