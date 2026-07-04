// SPDX-License-Identifier: Apache-2.0

//! Content-side in-memory ports: documents, members, oplog, audit.

use std::collections::HashMap;

use async_trait::async_trait;
use chrono::Utc;
use loki_crypto::WrappedDek;
use loki_model::{DocumentId, EncryptionTier, Role, UserId};
use loki_server_audit::{AuditAction, AuditEntry};

use crate::error::StoreError;
use crate::ports::{AuditStore, DocumentStore, MemberStore, OplogStore};
use crate::records::{DocMemberRecord, DocMetaRecord, OplogEntry};

use super::MemoryStores;

#[async_trait]
impl DocumentStore for MemoryStores {
    async fn create_document(&self, doc: &DocMetaRecord) -> Result<(), StoreError> {
        self.lock().documents.insert(doc.id, doc.clone());
        Ok(())
    }

    async fn get_document(&self, id: DocumentId) -> Result<Option<DocMetaRecord>, StoreError> {
        Ok(self.lock().documents.get(&id).cloned())
    }

    async fn set_snapshot(
        &self,
        id: DocumentId,
        ptr: &str,
        up_to: i64,
    ) -> Result<bool, StoreError> {
        let mut inner = self.lock();
        let doc = inner.documents.get_mut(&id).ok_or(StoreError::NotFound)?;
        // Move-forward-only, mirroring the Postgres guard (ADR-C013).
        if doc.snapshot_seq >= up_to {
            return Ok(false);
        }
        doc.snapshot_ptr = Some(ptr.to_owned());
        doc.snapshot_seq = up_to;
        Ok(true)
    }

    async fn set_tier(
        &self,
        id: DocumentId,
        tier: EncryptionTier,
        dek_wrapped: Option<&WrappedDek>,
    ) -> Result<(), StoreError> {
        let mut inner = self.lock();
        let doc = inner.documents.get_mut(&id).ok_or(StoreError::NotFound)?;
        doc.tier = tier;
        doc.dek_wrapped = dek_wrapped.cloned();
        Ok(())
    }

    async fn delete_document(&self, id: DocumentId) -> Result<(), StoreError> {
        let mut inner = self.lock();
        inner.documents.remove(&id).ok_or(StoreError::NotFound)?;
        inner.members.retain(|(doc, _), _| *doc != id);
        inner.oplog.retain(|e| e.doc_id != id);
        Ok(())
    }

    async fn shred_dek(&self, id: DocumentId) -> Result<(), StoreError> {
        let mut inner = self.lock();
        let doc = inner.documents.get_mut(&id).ok_or(StoreError::NotFound)?;
        doc.dek_wrapped = None;
        for member in inner.members.values_mut().filter(|m| m.doc_id == id) {
            member.dek_wrapped_for_user = None;
        }
        Ok(())
    }
}

#[async_trait]
impl MemberStore for MemoryStores {
    async fn upsert_member(&self, member: &DocMemberRecord) -> Result<(), StoreError> {
        self.lock()
            .members
            .insert((member.doc_id, member.user_id), member.clone());
        Ok(())
    }

    async fn get_member_role(
        &self,
        doc: DocumentId,
        user: UserId,
    ) -> Result<Option<Role>, StoreError> {
        Ok(self.lock().members.get(&(doc, user)).map(|m| m.role))
    }

    async fn list_members(&self, doc: DocumentId) -> Result<Vec<DocMemberRecord>, StoreError> {
        let inner = self.lock();
        let mut members: Vec<_> = inner
            .members
            .values()
            .filter(|m| m.doc_id == doc)
            .cloned()
            .collect();
        members.sort_by_key(|m| m.user_id.as_uuid());
        Ok(members)
    }

    async fn remove_member(&self, doc: DocumentId, user: UserId) -> Result<(), StoreError> {
        self.lock()
            .members
            .remove(&(doc, user))
            .map(|_| ())
            .ok_or(StoreError::NotFound)
    }
}

#[async_trait]
impl OplogStore for MemoryStores {
    async fn append(
        &self,
        doc: DocumentId,
        actor: UserId,
        payload: &[u8],
    ) -> Result<i64, StoreError> {
        let mut inner = self.lock();
        inner.next_seq += 1;
        let seq = inner.next_seq;
        inner.oplog.push(OplogEntry {
            doc_id: doc,
            seq,
            actor,
            payload: payload.to_vec(),
            created_at: Utc::now(),
        });
        Ok(seq)
    }

    async fn fetch_after(
        &self,
        doc: DocumentId,
        after: i64,
    ) -> Result<Vec<OplogEntry>, StoreError> {
        let inner = self.lock();
        Ok(inner
            .oplog
            .iter()
            .filter(|e| e.doc_id == doc && e.seq > after)
            .cloned()
            .collect())
    }

    async fn fetch_one(&self, doc: DocumentId, seq: i64) -> Result<Option<OplogEntry>, StoreError> {
        let inner = self.lock();
        Ok(inner
            .oplog
            .iter()
            .find(|e| e.doc_id == doc && e.seq == seq)
            .cloned())
    }

    async fn truncate_up_to(&self, doc: DocumentId, up_to: i64) -> Result<(), StoreError> {
        self.lock()
            .oplog
            .retain(|e| e.doc_id != doc || e.seq > up_to);
        Ok(())
    }

    async fn docs_with_backlog(
        &self,
        min_entries: i64,
    ) -> Result<Vec<(DocumentId, i64)>, StoreError> {
        let inner = self.lock();
        let mut counts: HashMap<DocumentId, i64> = HashMap::new();
        for entry in &inner.oplog {
            *counts.entry(entry.doc_id).or_default() += 1;
        }
        let mut backlog: Vec<_> = counts
            .into_iter()
            .filter(|(_, count)| *count >= min_entries)
            .collect();
        backlog.sort_by_key(|&(_, count)| std::cmp::Reverse(count));
        Ok(backlog)
    }
}

#[async_trait]
impl AuditStore for MemoryStores {
    async fn append_audit(
        &self,
        actor: &str,
        action: AuditAction,
        target: &str,
    ) -> Result<AuditEntry, StoreError> {
        let mut inner = self.lock();
        let entry = AuditEntry::append(inner.audit.last(), actor, action, target, Utc::now());
        inner.audit.push(entry.clone());
        Ok(entry)
    }

    async fn load_chain(&self) -> Result<Vec<AuditEntry>, StoreError> {
        Ok(self.lock().audit.clone())
    }
}
