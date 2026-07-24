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
use crate::file::{FileFilter, FileWriteError};

impl<B: MacroBackend> ExecutionHost<B> {
    /// `Application.OpenFileForReading([filter])` (spec §5.3): gate `FileRead`,
    /// raise the picker, and return a run-scoped file handle. A cancelled pick is
    /// a trappable error so a macro can `On Error` around it; a denied capability
    /// is the usual trappable permission error.
    pub(crate) fn open_file_for_reading(&mut self, args: &[Value]) -> Result<Value, RuntimeError> {
        self.gate(Capability::FileRead)?;
        let filter = filter_arg(args)?;
        match self.backend.read_file(&filter) {
            Some(picked) => Ok(Value::Object(self.doc.push_file(picked))),
            None => Err(pick_cancelled()),
        }
    }

    /// `Application.OpenFileForWriting([filter])` (spec §5.3): gate `FileWrite`,
    /// raise the **save** picker, and return a run-scoped write handle for the
    /// chosen target. A cancelled pick is a trappable error. No bytes are written
    /// until the macro `.Close`s the handle.
    pub(crate) fn open_file_for_writing(&mut self, args: &[Value]) -> Result<Value, RuntimeError> {
        self.gate(Capability::FileWrite)?;
        let filter = filter_arg(args)?;
        match self.backend.pick_write_target(&filter) {
            Some(path) => Ok(Value::Object(self.doc.push_write_file(path))),
            None => Err(pick_cancelled()),
        }
    }

    /// Flush a write handle's buffer to its picked target (its `.Close`). Closing
    /// an already-closed handle is a no-op; a write failure is a trappable error.
    pub(crate) fn close_write_file(&mut self, index: usize) -> Result<Value, RuntimeError> {
        let (path, bytes) = {
            let handle = self.doc.write_files.get_mut(index).ok_or_else(bad_handle)?;
            if handle.closed {
                return Ok(Value::Empty);
            }
            handle.closed = true;
            (handle.path.clone(), handle.buffer.clone().into_bytes())
        };
        match self.backend.write_file(&path, &bytes) {
            Ok(()) => Ok(Value::Empty),
            Err(e) => Err(write_failed(&e)),
        }
    }
}

/// The `filter` argument parsed from `args[0]`, or the "any file" default.
fn filter_arg(args: &[Value]) -> Result<FileFilter, RuntimeError> {
    match args.first() {
        Some(v) => Ok(FileFilter::parse(&v.to_basic_string()?)),
        None => Ok(FileFilter::default()),
    }
}

/// The trappable error a cancelled/empty file pick surfaces as (75 = path/file
/// access error, the closest VBA code).
fn pick_cancelled() -> RuntimeError {
    RuntimeError::new(75, "No file was chosen".to_string())
}

/// The trappable error a failed flush surfaces as.
fn write_failed(error: &FileWriteError) -> RuntimeError {
    match error {
        FileWriteError::Refused => {
            RuntimeError::new(75, "File writing is not available".to_string())
        }
        FileWriteError::Io(msg) => RuntimeError::new(75, format!("File write failed: {msg}")),
    }
}

/// An invalid write-file handle index (should not occur — the interpreter only
/// hands back handles this host minted).
fn bad_handle() -> RuntimeError {
    RuntimeError::new(
        438,
        "Object doesn't support this property or method".to_string(),
    )
}
