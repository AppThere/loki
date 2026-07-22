// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 AppThere Loki contributors

//! Network-capability policy (Phase 8 Track B; ADR-0015, 8B.1).
//!
//! The `Network` capability is unlike the others: its grant unit is an **origin**
//! (`scheme://host[:port]`), never a bare "network on" switch, and grants are
//! **session-max — never persisted to disk** (ADR-0015 §4.2; the sharpest
//! exfiltration edge, T4). [`NetworkPolicy`] holds the per-run enabled flag and
//! the origins already allowed. It is off by default and only ever enabled when
//! **both** the `macro-net` build feature ([`MACRO_NET_COMPILED`]) and the user's
//! runtime setting are on (ADR-0015 §6/§8).

use std::collections::BTreeSet;

/// Whether the `macro-net` build feature is compiled in. A distribution can
/// exclude the network code entirely by building without the feature; when it is
/// `false`, no runtime setting can enable network (ADR-0015 §8 decision 1). The
/// app must AND this with the user's runtime setting before enabling.
pub const MACRO_NET_COMPILED: bool = cfg!(feature = "macro-net");

/// Per-run network policy: whether network is enabled at all, and the set of
/// origins the user has already allowed this run/session.
///
/// Origins are opaque normalized strings (`https://api.example.com`), produced by
/// the request layer (8B.3). Nothing here persists — the caller keeps
/// `AllowSession` origins in session memory and **never** writes them to the
/// trust store (ADR-0015 §4.2).
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct NetworkPolicy {
    enabled: bool,
    origins: BTreeSet<String>,
}

impl NetworkPolicy {
    /// The default: network refused. Every run starts here unless the app
    /// explicitly enables it (feature + setting).
    #[must_use]
    pub fn disabled() -> Self {
        Self::default()
    }

    /// An enabled policy with no origins allowed yet.
    ///
    /// Callers **must** only enable when [`MACRO_NET_COMPILED`] is `true` *and*
    /// the user's runtime setting is on (ADR-0015 §8 decision 1); this type does
    /// not re-check the build feature so the origin logic stays unit-testable
    /// without it.
    #[must_use]
    pub fn enabled() -> Self {
        Self {
            enabled: true,
            origins: BTreeSet::new(),
        }
    }

    /// Whether network is enabled for this run at all.
    #[must_use]
    pub fn is_enabled(&self) -> bool {
        self.enabled
    }

    /// Whether `origin` is already allowed.
    #[must_use]
    pub fn allows(&self, origin: &str) -> bool {
        self.origins.contains(origin)
    }

    /// Allows an origin for this run (folded from a prompt answer or a
    /// session-remembered grant).
    pub fn allow_origin(&mut self, origin: impl Into<String>) {
        self.origins.insert(origin.into());
    }

    /// The allowed origins, for folding session grants into a fresh run.
    pub fn origins(&self) -> impl Iterator<Item = &str> {
        self.origins.iter().map(String::as_str)
    }
}

#[cfg(test)]
#[path = "net_tests.rs"]
mod tests;
