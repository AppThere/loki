// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 AppThere Loki contributors

//! The local trust store (macro spec §2.4).
//!
//! Trust lives *only* here — in the per-user profile, keyed by the hash of the
//! macro payload — never inside the document (threat T10). See [`store`] for
//! persistence and [`record`] for the per-document record shape.

pub(crate) mod hex;
mod record;
mod store;

pub use record::{PersistedGrant, TrustDecision, TrustRecord};
pub use store::TrustStore;
