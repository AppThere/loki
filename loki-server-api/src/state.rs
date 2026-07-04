// SPDX-License-Identifier: Apache-2.0

//! Shared handler state: every port the API layer uses.

use std::sync::Arc;

use loki_crypto::KeyWrap;
use loki_model::{EncryptionTier, Residency};
use loki_server_auth::IdentityVerifier;
use loki_server_collab::CollabState;
use loki_server_store::{BlobStore, Stores};

/// Ports and configuration shared by all request handlers.
#[derive(Clone)]
pub struct ApiState {
    /// Persistence ports (Postgres in production, in-memory in tests).
    pub stores: Stores,
    /// Snapshot/attachment storage.
    pub blob: BlobStore,
    /// Collaboration relay state (oplog + fan-out bus).
    pub collab: CollabState,
    /// Bearer-token verification (ADR-C017).
    pub verifier: Arc<dyn IdentityVerifier>,
    /// Wraps new document DEKs under the deployment's Tier-0/1 KEK
    /// (ADR-C014). Tier-2 documents never use this — their DEKs are
    /// client-held.
    pub tier_kek: Arc<dyn KeyWrap>,
    /// Residency recorded on new workspaces/documents (ADR-C019).
    pub residency: Residency,
    /// Deployment default tier for new workspaces (ratified decision §6.1:
    /// Tier 0 for SaaS, Tier 1 for enterprise self-host; Tier 2 is always
    /// per-document opt-in).
    pub default_tier: EncryptionTier,
}
