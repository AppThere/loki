// SPDX-License-Identifier: Apache-2.0

//! Real-time collaboration relay (ADR-C012 / ADR-C013).
//!
//! Clients exchange opaque Loro update frames over a per-document WebSocket.
//! The server appends updates to the oplog, broadcasts them to connected
//! members, and fans out across instances through the [`FanOutBus`] port:
//!
//! - [`InMemoryBus`] — single-process (tests, embedded use).
//! - [`PgNotifyBus`] — Postgres `LISTEN`/`NOTIFY`, the zero-extra-infra
//!   default (ADR-C012). A `RedisBus` slots in later for large clusters.
//!
//! The server never merges or interprets CRDT semantics — under Tier 2 every
//! payload is AEAD ciphertext produced client-side (ADR-C014), including
//! awareness (ratified decision §6.3).

#![forbid(unsafe_code)]

mod bus;
mod bus_memory;
mod bus_pg;
mod compact;
mod hub;
mod msg;
mod relay;
mod ws;

pub use bus::{BusError, BusEvent, FanOutBus, Origin};
pub use bus_memory::InMemoryBus;
pub use bus_pg::PgNotifyBus;
pub use compact::{CompactError, CompactionOutcome, Compactor};
pub use msg::{CollabFrame, FrameError};
pub use relay::{CollabState, DocRelay, RelayError};
pub use ws::drive_socket;
