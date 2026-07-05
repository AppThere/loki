// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 AppThere Loki contributors

//! Client-side CRDT history compaction (memory-audit Finding 6 /
//! `TODO(loro-compaction)`).
//!
//! A Loro oplog grows with every edit — with session *time*, not document
//! size — so a long editing session retains unbounded history. Two remedies,
//! in increasing strength:
//!
//! - [`compact_in_place`]: re-encode parsed ops into Loro's compact change
//!   store and drop checkout caches. No history is lost, undo and
//!   subscriptions keep working; only the memory *representation* shrinks.
//!   Safe to call at any quiescent point (e.g. after every save).
//! - [`compact_history`]: export a minimal-history snapshot of the current
//!   state and import it into a fresh, mark-configured [`LoroDoc`]. This
//!   truncates the oplog itself — the durable fix — at the cost that
//!   anything bound to the old doc instance (an `UndoManager`, container
//!   handles, subscriptions, an `IncrementalReader` seed) must be recreated
//!   by the caller, and undo history restarts at the compaction point.
//!   Client-local documents only: a peer that is behind the shallow start
//!   can no longer sync updates (server-relayed Tier-0/1 documents are
//!   compacted by the ADR-C013 server `Compactor`; Tier-2 by the
//!   `PUT /snapshot` flow).

use super::BridgeError;
use crate::loro_schema::{CHAR_MARK_KEYS, INLINE_OBJECT_MARK_KEYS};
use loro::{ExpandType, ExportMode, LoroDoc, StyleConfig, StyleConfigMap};

/// Registers every schema mark key's expand behaviour on `doc`.
///
/// Must run on each fresh [`LoroDoc`] before it carries document text —
/// [`super::document_to_loro`] and [`compact_history`] both call it. The
/// config is per-instance runtime state; it does not travel in snapshots.
pub(super) fn configure_text_style(doc: &LoroDoc) {
    let mut style_config = StyleConfigMap::new();
    // Character formatting marks expand onto text inserted at their trailing
    // edge (`After`) — the single source of truth is `CHAR_MARK_KEYS`.
    for key in CHAR_MARK_KEYS {
        style_config.insert(
            loro::InternalString::from(*key),
            StyleConfig {
                expand: ExpandType::After,
            },
        );
    }
    // Inline-object anchor marks must not expand onto adjacent text — they
    // describe a single placeholder position, not a formatting span.
    for key in INLINE_OBJECT_MARK_KEYS {
        style_config.insert(
            loro::InternalString::from(*key),
            StyleConfig {
                expand: ExpandType::None,
            },
        );
    }
    doc.config_text_style(style_config);
}

/// In-place memory compaction: commit, re-encode the change store, and free
/// checkout caches. History (and therefore undo) is fully preserved.
pub fn compact_in_place(doc: &LoroDoc) {
    doc.commit();
    doc.compact_change_store();
    doc.free_history_cache();
    doc.free_diff_calculator();
}

/// History truncation: returns a **fresh** [`LoroDoc`] holding the same
/// document state with a minimal-depth oplog (Loro `StateOnly` shallow
/// snapshot at the latest version).
///
/// The caller must swap the returned doc in for the old one and recreate
/// everything bound to the old instance — see the module docs for the list
/// and the sync caveat.
pub fn compact_history(doc: &LoroDoc) -> Result<LoroDoc, BridgeError> {
    doc.commit();
    let bytes = doc
        .export(ExportMode::StateOnly(None))
        .map_err(|e| BridgeError::Loro(e.to_string()))?;
    let fresh = LoroDoc::new();
    configure_text_style(&fresh);
    fresh.import(&bytes)?;
    Ok(fresh)
}
