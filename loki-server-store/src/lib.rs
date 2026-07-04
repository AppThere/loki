// SPDX-License-Identifier: Apache-2.0

//! Persistence ports for the Loki server (ADR-C016).
//!
//! PostgreSQL is the system-of-record (workspaces, users, ACLs, `doc_meta`,
//! the hot `doc_oplog`, the audit chain); object storage holds snapshots and
//! attachments. Every store is a trait ("port") with two implementations:
//!
//! - [`pg`] — SQLx/Postgres, the production path.
//! - [`memory`] — in-process, for unit tests and API-handler tests.
//!
//! The blob side ([`BlobStore`]) wraps `object_store`, so Hetzner Object
//! Storage and MinIO are a config-URL swap, not a code fork.

#![forbid(unsafe_code)]

mod blob;
mod error;
pub mod memory;
pub mod pg;
mod ports;
mod records;

pub use blob::BlobStore;
pub use error::StoreError;
pub use ports::{
    AuditStore, DocumentStore, MemberStore, OplogStore, Stores, UserStore, WorkspaceStore,
};
pub use records::{DocMemberRecord, DocMetaRecord, OplogEntry, UserRecord, WorkspaceRecord};
