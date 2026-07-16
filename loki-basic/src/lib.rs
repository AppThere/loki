// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 AppThere Loki contributors

#![forbid(unsafe_code)]
#![warn(missing_docs)]
#![warn(clippy::pedantic)]
#![allow(clippy::module_name_repetitions)]
// A numeric interpreter converts between BASIC's numeric domains constantly
// (Integer/Long/Double ↔ i64/f64). These conversions are deliberate and, where
// truncation would be observable, guarded by explicit range checks (`to_i16`/
// `to_i32` via `try_from`) or documented saturation. Blanket-allow the noisy
// numeric-cast pedantic lints rather than pepper the arithmetic with attributes.
#![allow(
    clippy::cast_precision_loss,
    clippy::cast_possible_truncation,
    clippy::cast_sign_loss,
    clippy::cast_possible_wrap
)]
// Exact float comparisons here are deliberate: divide-by-zero and is-zero
// guards test against exactly `0.0`. Value equality (the BASIC `=` operator)
// uses `partial_cmp`, not `==`, so it is unaffected.
#![allow(clippy::float_cmp)]

//! `loki-basic` — a pure, tree-walking BASIC interpreter for the Loki suite.
//!
//! This crate implements the language surface shared by **VBA** (MS-VBAL) and
//! **`StarBasic`** (`OpenOffice` Basic) over a single AST, selected by a
//! [`Dialect`] flag. It is the execution engine described in the macro
//! scripting spec (`docs/adr/LOKI_MACRO_SCRIPTING_SPEC.md`), Phase 2.
//!
//! # Security posture
//!
//! - **No JIT, no codegen** — interpretation only, so the engine is compatible
//!   with platforms that forbid runtime code generation (iOS `W^X`).
//! - **No ambient authority** — the interpreter can evaluate expressions and
//!   mutate its own heap, but **every** observable effect (dialogs, document
//!   reads/writes, files, …) is routed through the [`Host`](host::Host) trait.
//!   A `loki-basic` embedded with an empty host is a pure calculator; there is
//!   no intrinsic I/O function and no escape hatch to add one from script.
//! - **No I/O dependencies** — the crate links nothing that can reach the
//!   filesystem, network, or process table (enforced in CI).
//!
//! # Status
//!
//! Phase 2 builds the language core (lexer, parser, value model, evaluator,
//! fuel metering, host seam, pure-compute standard library). The capability
//! broker, trust store, and object-model facades live in `loki-macro-host`
//! (later phases) and are **not** part of this crate.

pub mod ast;
pub mod dialect;
pub mod error;
pub mod host;
pub mod interp;
pub mod lexer;
pub mod parser;
pub mod value;

pub use interp::Interp;
pub use value::Value;

pub use dialect::Dialect;
pub use error::{BasicError, RuntimeError, Span};
pub use host::{FuelVerdict, Host};
