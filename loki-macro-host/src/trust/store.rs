// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 AppThere Loki contributors

//! [`TrustStore`] — the per-user, local, on-disk registry of trust decisions
//! (macro spec §2.4).
//!
//! The store is keyed by the macro-payload hash. It is the *only* place a trust
//! decision comes from: nothing in a document can add, remove, or alter a record
//! (threat T10). Persistence is best-effort — a missing or corrupt store simply
//! means "no documents are trusted yet", so a broken file never blocks opening a
//! document.

use std::collections::BTreeMap;
use std::path::PathBuf;

use serde::{Deserialize, Serialize};

use super::record::{Provenance, TrustDecision, TrustRecord, now_secs};
use crate::error::MacroHostError;

/// On-disk schema version, so future format changes are detectable.
const STORE_VERSION: u32 = 1;

/// The serialized store: a version tag plus records keyed by hex payload hash.
#[derive(Debug, Serialize, Deserialize, Default)]
struct StoreFile {
    version: u32,
    #[serde(default)]
    records: BTreeMap<String, TrustRecord>,
}

/// The in-memory trust store with optional on-disk backing.
///
/// Records are keyed by the 32-byte payload hash. Only *persistent* decisions
/// ([`TrustDecision::is_persistent`]) are written to disk on [`Self::save`];
/// session-only trust lives in `MacroService`, never here.
#[derive(Debug, Default)]
pub struct TrustStore {
    records: BTreeMap<[u8; 32], TrustRecord>,
    /// Where the store persists (`None` = in-memory only, e.g. tests).
    path: Option<PathBuf>,
}

impl TrustStore {
    /// Creates an empty store that persists to `path` (or stays in-memory when
    /// `None`).
    #[must_use]
    pub fn new(path: Option<PathBuf>) -> Self {
        Self {
            records: BTreeMap::new(),
            path,
        }
    }

    /// Loads the store from `path`, returning an empty store bound to that path
    /// if the file is absent. A present-but-corrupt file yields
    /// [`MacroHostError::Corrupt`]; callers that prefer robustness over
    /// diagnostics can use [`Self::load_or_empty`].
    ///
    /// # Errors
    ///
    /// [`MacroHostError::Io`] on a read error other than "not found";
    /// [`MacroHostError::Corrupt`] if the file cannot be parsed.
    pub fn load(path: PathBuf) -> Result<Self, MacroHostError> {
        let text = match std::fs::read_to_string(&path) {
            Ok(t) => t,
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
                return Ok(Self::new(Some(path)));
            }
            Err(source) => return Err(MacroHostError::Io { path, source }),
        };
        let parsed: StoreFile =
            serde_json::from_str(&text).map_err(|e| MacroHostError::Corrupt {
                path: path.clone(),
                reason: e.to_string(),
            })?;
        let mut records = BTreeMap::new();
        for rec in parsed.records.into_values() {
            records.insert(rec.doc_key, rec);
        }
        Ok(Self {
            records,
            path: Some(path),
        })
    }

    /// [`Self::load`] but a corrupt or unreadable file degrades to an empty
    /// store (still bound to `path`, so the next [`Self::save`] rewrites it).
    /// Preferred at app startup where a broken store must never block opening.
    #[must_use]
    pub fn load_or_empty(path: PathBuf) -> Self {
        Self::load(path.clone()).unwrap_or_else(|_| Self::new(Some(path)))
    }

    /// The record for `key`, if any.
    #[must_use]
    pub fn get(&self, key: &[u8; 32]) -> Option<&TrustRecord> {
        self.records.get(key)
    }

    /// The record for `key`, mutably.
    pub fn get_mut(&mut self, key: &[u8; 32]) -> Option<&mut TrustRecord> {
        self.records.get_mut(key)
    }

    /// Inserts or replaces a record.
    pub fn insert(&mut self, record: TrustRecord) {
        self.records.insert(record.doc_key, record);
    }

    /// Re-keys the record from `old_key` to `new_key` and marks it
    /// [`Provenance::AuthoredHere`] — the trust half of an **in-app macro edit**
    /// (spec §2.4/§2.5). When the editor writes back edited source the payload
    /// hash changes; this carries the existing decision, grants, and auto-run
    /// opt-in over to the new hash so the user's own edit keeps trust.
    ///
    /// Returns `true` if a record existed at `old_key` and was moved. When no
    /// record exists it returns `false` and creates nothing — trust is never
    /// fabricated, so an untrusted document stays untrusted after an edit, and an
    /// *external* modification (which never calls this) still drops to untrusted
    /// via the plain hash mismatch. If `old_key == new_key` (a no-op edit) the
    /// existing record is marked authored in place.
    pub fn reauthor(&mut self, old_key: &[u8; 32], new_key: [u8; 32]) -> bool {
        let Some(mut record) = self.records.remove(old_key) else {
            return false;
        };
        record.doc_key = new_key;
        record.provenance = Provenance::AuthoredHere;
        record.last_used = now_secs();
        self.records.insert(new_key, record);
        true
    }

    /// Removes the record for `key` — "forget this document" (spec §9.4).
    /// Returns the removed record, if any.
    pub fn forget(&mut self, key: &[u8; 32]) -> Option<TrustRecord> {
        self.records.remove(key)
    }

    /// All records, for the management list (spec §9.4). Ordered by key for
    /// stable display.
    pub fn records(&self) -> impl Iterator<Item = &TrustRecord> {
        self.records.values()
    }

    /// Number of records held.
    #[must_use]
    pub fn len(&self) -> usize {
        self.records.len()
    }

    /// Whether the store holds no records.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.records.is_empty()
    }

    /// The configured persistence path, if any.
    #[must_use]
    pub fn path(&self) -> Option<&PathBuf> {
        self.path.as_ref()
    }

    /// Writes the persistent records to disk (creating parent directories).
    /// Session-only records are skipped (spec §2.3). A store with no path is a
    /// no-op success (in-memory mode).
    ///
    /// # Errors
    ///
    /// [`MacroHostError::Io`] if the directory or file cannot be written, or the
    /// records cannot be serialized.
    pub fn save(&self) -> Result<(), MacroHostError> {
        let Some(path) = &self.path else {
            return Ok(());
        };
        let mut file = StoreFile {
            version: STORE_VERSION,
            records: BTreeMap::new(),
        };
        for rec in self.records.values() {
            if rec.decision.is_persistent() {
                file.records
                    .insert(super::hex::encode(&rec.doc_key), rec.clone());
            }
        }
        let json = serde_json::to_string_pretty(&file).map_err(|e| MacroHostError::Io {
            path: path.clone(),
            source: std::io::Error::other(e),
        })?;
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent).map_err(|source| MacroHostError::Io {
                path: parent.to_path_buf(),
                source,
            })?;
        }
        std::fs::write(path, json).map_err(|source| MacroHostError::Io {
            path: path.clone(),
            source,
        })
    }

    /// Convenience: the standing decision for `key`, defaulting to
    /// [`TrustDecision::Disabled`] when no record exists — the safe default that
    /// makes trust impossible to infer from a document's own content (T10).
    #[must_use]
    pub fn decision(&self, key: &[u8; 32]) -> TrustDecision {
        self.records
            .get(key)
            .map(|r| r.decision)
            .unwrap_or_default()
    }
}

#[cfg(test)]
#[path = "store_tests.rs"]
mod tests;
