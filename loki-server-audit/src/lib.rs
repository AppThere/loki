// SPDX-License-Identifier: Apache-2.0

//! Append-only, hash-chained audit log (ADR-C020).
//!
//! Each entry carries the SHA-256 hash of the previous entry plus its own
//! hash over a canonical, length-prefixed encoding — so any insertion,
//! deletion, or mutation breaks the chain and is detected by
//! [`verify_chain`]. Persistence lives in `loki-server-store`; this crate is
//! the pure chain logic so it can also run client-side for verification.

#![forbid(unsafe_code)]

mod action;
mod entry;

pub use action::AuditAction;
pub use entry::{verify_chain, AuditEntry, ChainError, HASH_LEN};
