// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 AppThere Loki contributors

//! XLSX import and export modules.

pub mod export;
pub mod import;

#[cfg(test)]
#[path = "vba_tests.rs"]
mod vba_tests;
