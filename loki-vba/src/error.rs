// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 AppThere Loki contributors

//! Typed errors for VBA source extraction.
//!
//! Every failure is a *typed* error, never a panic (macro spec §4.4, T9): a
//! malformed or hostile `vbaProject.bin` degrades to "macros unreadable —
//! preserved but cannot be enabled", it never crashes the reader.

use thiserror::Error;

/// A failure while reading a VBA project.
#[derive(Debug, Error, Clone, PartialEq, Eq)]
pub enum VbaError {
    /// The container is not a valid OLE compound file, or a required storage /
    /// stream is missing.
    #[error("VBA container is not readable: {0}")]
    Container(String),

    /// The MS-OVBA compressed stream is malformed.
    #[error("VBA compressed stream is malformed: {0}")]
    Compression(String),

    /// The `dir` stream (project descriptor) could not be parsed.
    #[error("VBA project directory is malformed: {0}")]
    Directory(String),

    /// A decompression bomb guard tripped — the stream expanded past the cap.
    #[error("VBA stream exceeded the decompression size cap")]
    TooLarge,

    /// Writing an edited project back to a container failed (source-only
    /// write-back, macro spec §3.4).
    #[error("VBA container could not be written: {0}")]
    Write(String),

    /// Edited source for a module contains characters that cannot be stored in
    /// the project's code page — saving it would silently corrupt the source, so
    /// the write is refused (macro spec §3.4). The payload holds the module name.
    #[error("module {0} has characters that cannot be stored in the project's code page")]
    Encoding(String),
}

/// Convenience result alias.
pub type VbaResult<T> = Result<T, VbaError>;
