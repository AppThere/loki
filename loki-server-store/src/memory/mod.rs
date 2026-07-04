// SPDX-License-Identifier: Apache-2.0

//! In-memory implementation of every port — for tests and API-handler tests.
//!
//! Not for production use: nothing is durable. The implementation mirrors the
//! Postgres semantics (upsert behaviour, cascade on delete, chained audit,
//! move-forward-only snapshot pointer). Split: identity-side ports
//! (workspaces/users) live here; content-side ports (documents, members,
//! oplog, audit) in [`content`].

mod content;

use std::collections::HashMap;
use std::sync::{Arc, Mutex};

use async_trait::async_trait;
use loki_model::{DocumentId, UserId, WorkspaceId};
use loki_server_audit::AuditEntry;

use crate::error::StoreError;
use crate::ports::{Stores, UserStore, WorkspaceStore};
use crate::records::{DocMemberRecord, DocMetaRecord, OplogEntry, UserRecord, WorkspaceRecord};

#[derive(Default)]
pub(super) struct Inner {
    pub(super) workspaces: HashMap<WorkspaceId, WorkspaceRecord>,
    pub(super) users: HashMap<UserId, UserRecord>,
    pub(super) documents: HashMap<DocumentId, DocMetaRecord>,
    pub(super) members: HashMap<(DocumentId, UserId), DocMemberRecord>,
    pub(super) oplog: Vec<OplogEntry>,
    pub(super) next_seq: i64,
    pub(super) audit: Vec<AuditEntry>,
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

    pub(super) fn lock(&self) -> std::sync::MutexGuard<'_, Inner> {
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
        docs.sort_by_key(|d| std::cmp::Reverse(d.created_at));
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
