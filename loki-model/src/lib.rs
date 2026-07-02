// SPDX-License-Identifier: Apache-2.0

//! Shared server-side domain types for the Loki suite.
//!
//! This crate holds the identifier newtypes, the tiered-confidentiality model
//! (ADR-C014), the RBAC role/action matrix (ADR-C017), and the data-residency
//! type (ADR-C019) shared between `loki-server-*` crates and future clients.
//! It deliberately has **no** server dependencies (no Axum, no SQLx) so that
//! client crates can depend on it too.
//!
//! See `docs/adr/LOKI_WEB_SERVER_SPEC.md`.

#![forbid(unsafe_code)]

mod ids;
mod residency;
mod role;
mod tier;

pub use ids::{DocumentId, UserId, WorkspaceId};
pub use residency::{Residency, ResidencyError};
pub use role::{Action, Role, RoleParseError};
pub use tier::{EncryptionTier, TierParseError};
