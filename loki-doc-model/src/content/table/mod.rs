// Copyright 2026 AppThere Loki contributors
// SPDX-License-Identifier: Apache-2.0

//! Table model types.
//!
//! Modelled on pandoc's table model (pandoc-types ≥ 2.11):
//! `Table Attr Caption [ColSpec] TableHead [TableBody] TableFoot`.
//! TR 29166 §6.2.4 and §7.2.4.

pub mod col;
pub mod core;
pub mod row;

pub use col::{ColAlignment, ColSpec, ColWidth};
pub use core::{Table, TableBody, TableCaption, TableFoot, TableHead};
pub use row::{Cell, CellProps, CellVerticalAlign, Row};
