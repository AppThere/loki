// Copyright 2026 AppThere Loki contributors
// SPDX-License-Identifier: MIT

//! ZIP container I/O for OPC packages.
//!
//! [`mod@read`] deserialises a [`crate::package::Package`] from a ZIP archive
//! and [`mod@write`] serialises one back out. The `limits` module holds the
//! decompression budgets that guard both paths against zip-bomb inputs.

pub(crate) mod limits;
pub mod read;
pub mod write;
