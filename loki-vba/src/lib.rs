// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 AppThere Loki contributors

#![forbid(unsafe_code)]
#![warn(missing_docs)]
#![warn(clippy::pedantic)]
#![allow(clippy::module_name_repetitions)]

//! `loki-vba` — extract **source text only** from a VBA project
//! (`vbaProject.bin`).
//!
//! Real-world OOXML macro documents store VBA as an OLE compound file. Loki
//! reads it so a user can *see* what a macro does before deciding to trust it
//! (macro spec §9.6, "visibility before executability"), but it does so under a
//! hard rule (§4.4, threat T5 — "VBA stomping"):
//!
//! > **The compiled p-code, `PerformanceCache`, and `_VBA_PROJECT` streams are
//! > never parsed and never executed.** Only the decompressed *source text* is
//! > read. A module whose source has been wiped while live p-code remains is
//! > treated as an empty module, and the mismatch raises a tamper warning.
//!
//! Everything here is a pure byte transform over an in-memory buffer — no
//! filesystem, no network, no code execution. Malformed or hostile input
//! degrades to a typed [`VbaError`], never a panic (§12, T9).

mod compress;
mod decompress;
mod dir;
mod project;
mod tamper;

pub mod error;

pub use error::{VbaError, VbaResult};
pub use project::{ModuleKind, VbaModule, VbaProject};

// Re-exported for the fuzz harness and integration tests.
#[doc(hidden)]
pub use compress::compress;
#[doc(hidden)]
pub use decompress::decompress;
