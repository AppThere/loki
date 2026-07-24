// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 AppThere Loki contributors

//! The file verb of the execution host (macro spec §5.3, Phase 7B), split from
//! `exec/mod.rs` for the 300-line ceiling.
//!
//! `Application.OpenFileForReading(filter)` gates the [`Capability::FileRead`]
//! capability, then raises the OS picker through the app's [`MacroBackend`]. The
//! user's pick is the second consent (T3) — there is no path-addressed API, so a
//! macro can only ever touch a file the user handed it. The returned handle is
//! run-scoped and read-only.

use loki_basic::{RuntimeError, Value};

use super::{ExecutionHost, MacroBackend};
use crate::capability::Capability;
use crate::file::FileFilter;

impl<B: MacroBackend> ExecutionHost<B> {
    /// `Application.OpenFileForReading([filter])` (spec §5.3): gate `FileRead`,
    /// raise the picker, and return a run-scoped file handle. A cancelled pick is
    /// a trappable error so a macro can `On Error` around it; a denied capability
    /// is the usual trappable permission error.
    pub(crate) fn open_file_for_reading(&mut self, args: &[Value]) -> Result<Value, RuntimeError> {
        self.gate(Capability::FileRead)?;
        let filter = match args.first() {
            Some(v) => FileFilter::parse(&v.to_basic_string()?),
            None => FileFilter::default(),
        };
        match self.backend.read_file(&filter) {
            Some(picked) => Ok(Value::Object(self.doc.push_file(picked))),
            None => Err(pick_cancelled()),
        }
    }
}

/// The trappable error a cancelled/empty file pick surfaces as (75 = path/file
/// access error, the closest VBA code).
fn pick_cancelled() -> RuntimeError {
    RuntimeError::new(75, "No file was chosen".to_string())
}
