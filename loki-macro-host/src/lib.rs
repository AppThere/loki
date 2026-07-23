// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 AppThere Loki contributors

//! Macro trust & capability infrastructure for the Loki suite (macro spec
//! Phase 4).
//!
//! This crate is the security spine of macro support. It does **not** execute
//! anything (execution wiring is Phase 5); it decides *whether* a document's
//! macros may run at all and, once running, *which effects* they may perform:
//!
//! - [`TrustStore`] — the per-user, local, on-disk record of which documents
//!   the user has enabled, keyed by the **hash of the macro payload** (spec
//!   §2.4). Nothing inside a document can influence its own trust (threat T10):
//!   a document that claims "I am trusted" in its own bytes is ignored, because
//!   trust is looked up in the local profile, never read from the file.
//! - [`Capability`] / [`GrantScope`] — the closed catalog of effects a macro
//!   might request and the scopes at which the user can grant them (spec §5).
//! - [`CapabilityBroker`] — the [`loki_basic::Host`] implementation that gates
//!   every requested effect against the grant table (spec §4.3, §5.1). The
//!   interpreter has no ambient authority; the broker *is* the authority.
//! - [`MacroService`] — the cheap-to-clone handle apps provide into their
//!   Dioxus context (the `SpellService` pattern), wrapping the trust store and
//!   per-open-document session state.
//!
//! Layer L5 (ADR-0009): depends on the interpreter core ([`loki_basic`]) and the
//! neutral document models, and is kept out of every server/headless crate's
//! dependency graph (spec §10) so macros can never execute server-side.

#![forbid(unsafe_code)]

pub mod broker;
pub mod capability;
pub mod error;
pub mod events;
pub mod exec;
pub mod http;
pub mod net;
pub mod runtime;
pub mod service;
pub mod trust;
pub mod verify;

/// Re-exported so consumers can name the dialect for [`runtime::MacroRuntime`],
/// implement [`exec::MacroBackend`], and pass/receive interpreter values
/// (UDF args/results) without depending on `loki-basic` directly.
pub use loki_basic::{Dialect, DialogKind, DialogRequest, Value};

pub use broker::{CapabilityBroker, GrantSet};
pub use capability::{Capability, CapabilityDecision, GrantScope, RunContext};
pub use error::MacroHostError;
pub use events::{EventPhase, auto_open_handlers, handler_phase, is_auto_open};
pub use exec::{DenyBackend, DialogOutcome, DocEdit, EditBatch, ExecutionHost, MacroBackend};
pub use http::{HttpError, HttpRequest, HttpResponse, origin_of};
pub use net::{MACRO_NET_COMPILED, NetworkPolicy};
pub use runtime::{AutoRunToken, MacroRunError, MacroRuntime, RunOutcome, RunRequest, UdfOutcome};
pub use service::{
    CapabilityState, DocumentSecurity, MacroService, SignatureStatus, SignatureSummary,
};
pub use trust::{
    PersistedGrant, Provenance, PublisherRecord, TrustDecision, TrustRecord, TrustStore,
    TrustedPublisherStore,
};
pub use verify::verify_payload;
