// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 AppThere Loki contributors

//! Spreadsheet document model and Loro CRDT bridge.

#![forbid(unsafe_code)]

pub mod loro_bridge;
pub mod workbook;

pub use loro_bridge::{BridgeError, loro_to_workbook, workbook_to_loro};
pub use workbook::{Cell, CellAlign, CellStyle, DocumentMeta, NumberFormat, Workbook, Worksheet};
