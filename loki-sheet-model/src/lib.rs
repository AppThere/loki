// Copyright 2026 AppThere Loki contributors
// SPDX-License-Identifier: Apache-2.0

//! Spreadsheet document model and Loro CRDT bridge.

pub mod workbook;
pub mod loro_bridge;

pub use workbook::{Workbook, Worksheet, Cell, CellStyle, CellAlign, NumberFormat, DocumentMeta};
pub use loro_bridge::{workbook_to_loro, loro_to_workbook, BridgeError};
