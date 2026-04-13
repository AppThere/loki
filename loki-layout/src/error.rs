// Copyright 2026 AppThere Loki contributors
// SPDX-License-Identifier: Apache-2.0

//! Error types for `loki-layout`.

use thiserror::Error;

/// Errors that can occur during document layout.
#[non_exhaustive]
#[derive(Debug, Error)]
pub enum LayoutError {
    /// A required font could not be loaded.
    #[error("font loading failed: {0}")]
    FontLoad(String),

    /// The layout algorithm encountered an unrecoverable condition.
    #[error("layout failed: {reason}")]
    Layout {
        /// Human-readable description of the failure.
        reason: String,
    },
}

/// Convenience alias for `Result<T, LayoutError>`.
pub type LayoutResult<T> = Result<T, LayoutError>;
