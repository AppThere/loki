// SPDX-License-Identifier: Apache-2.0

//! In-memory implementation of every port — for tests and API-handler tests.
//!
//! Not for production use: nothing is durable. The implementation mirrors the
//! Postgres semantics (upsert behaviour, cascade on delete, chained audit).

use std::collections::HashMap;
use std::sync::{Arc, Mutex};

use async_trait::async_trait;
use chrono::Utc;
use loki_crypto::WrappedDek;
use loki_model::{DocumentId, EncryptionTier, Role, UserId, WorkspaceId};
use loki_server_audit::{AuditAction, AuditEntry};

use crate::error::StoreError;
use crate::ports::{
    AuditStore, DocumentStore, MemberStore, OplogStore, Stores, UserStore, WorkspaceStore,
};
use crate::records::{DocMemberRecord, DocMetaRecord, OplogEntry, UserRecord, WorkspaceRecord};

#[derive(Default)]
struct Inner {
    workspaces: HashMap<WorkspaceId, WorkspaceRecord>,
    users: HashMap<UserId, UserRecord>,
    documents: HashMap<DocumentId, DocMetaRecord>,
    members: HashMap<(DocumentId, UserId), DocMemberRecord>,
    oplog: Vec<OplogEntry>,
    next_seq: i64,
    audit: Vec<AuditEntry>,
}

/// All six ports backed by one in-process state.
#[derive(Clone, Default)]
pub struct MemoryStores {
    inner: Arc<Mutex<Inner>>,
}

impl MemoryStores {
    /// Creates an empty store set.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Bundles this instance into a [`Stores`] aggregate.
    #[must_use]
    pub fn into_stores(self) -> Stores {
        Stores {
            workspaces: Arc::new(self.clone()),
            users: Arc::new(self.clone()),
            documents: Arc::new(self.clone()),
            members: Arc::new(self.clone()),
            oplog: Arc::new(self.clone()),
            audit: Arc::new(self),
        }
    }

    fn lock(&self) -> std::sync::MutexGuard<'_, Inner> {
        // A poisoned mutex means a test already panicked; propagate the state.
        match self.inner.lock() {
            Ok(guard) => guard,
            Err(poisoned) => poisoned.into_inner(),
        }
    }
}

#[async_trait]
impl WorkspaceStore for MemoryStores {
    async fn create_workspace(&self, workspace: &WorkspaceRecord) -> Result<(), StoreError> {
        self.lock()
            .workspaces
            .insert(workspace.id, workspace.clone());
        Ok(())
    }

    async fn get_workspace(&self, id: WorkspaceId) -> Result<Option<WorkspaceRecord>, StoreError> {
        Ok(self.lock().workspaces.get(&id).cloned())
    }

    async fn list_documents(&self, id: WorkspaceId) -> Result<Vec<DocMetaRecord>, StoreError> {
        let inner = self.lock();
        let mut docs: Vec<_> = inner
            .documents
            .values()
            .filter(|d| d.workspace_id == id)
            .cloned()
            .collect();
        docs.sort_by(|a, b| b.created_at.cmp(&a.created_at));
        Ok(docs)
    }
}

#[async_trait]
impl UserStore for MemoryStores {
    async fn upsert_user_by_oidc(
        &self,
        oidc_sub: &str,
        display_name: &str,
    ) -> Result<UserRecord, StoreError> {
        let mut inner = self.lock();
        if let Some(user) = inner.users.values().find(|u| u.oidc_sub == oidc_sub) {
            return Ok(user.clone());
        }
        let user = UserRecord {
            id: UserId::new(),
            oidc_sub: oidc_sub.to_owned(),
            display_name: display_name.to_owned(),
            public_key: None,
        };
        inner.users.insert(user.id, user.clone());
        Ok(user)
    }

    async fn get_user(&self, id: UserId) -> Result<Option<UserRecord>, StoreError> {
        Ok(self.lock().users.get(&id).cloned())
    }

    async fn set_public_key(&self, id: UserId, public_key: &[u8]) -> Result<(), StoreError> {
        let mut inner = self.lock();
        let user = inner.users.get_mut(&id).ok_or(StoreError::NotFound)?;
        user.public_key = Some(public_key.to_vec());
        Ok(())
    }

    async fn anonymize_user(&self, id: UserId) -> Result<(), StoreError> {
        let mut inner = self.lock();
        let user = inner.users.get_mut(&id).ok_or(StoreError::NotFound)?;
        user.display_name = String::new();
        user.oidc_sub = format!("erased:{id}");
        user.public_key = None;
        Ok(())
    }
}

#[async_trait]
impl DocumentStore for MemoryStores {
    async fn create_document(&self, doc: &DocMetaRecord) -> Result<(), StoreError> {
        self.lock().documents.insert(doc.id, doc.clone());
        Ok(())
    }

    async fn get_document(&self, id: DocumentId) -> Result<Option<DocMetaRecord>, StoreError> {
        Ok(self.lock().documents.get(&id).cloned())
    }

    async fn set_snapshot_ptr(&self, id: DocumentId, ptr: &str) -> Result<(), StoreError> {
        let mut inner = self.lock();
        let doc = inner.documents.get_mut(&id).ok_or(StoreError::NotFound)?;
        doc.snapshot_ptr = Some(ptr.to_owned());
        Ok(())
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
