// Copyright 2026 AppThere Loki contributors
// SPDX-License-Identifier: Apache-2.0

//! Error types for the `loki-vello` rendering backend.

/// Errors that can occur while translating a [`loki_layout::DocumentLayout`] to a Vello scene.
#[non_exhaustive]
#[derive(Debug, thiserror::Error)]
pub enum VelloError {
    /// Image data could not be decoded.
    #[error("image decode failed: {reason}")]
    ImageDecode {
        /// Human-readable description of the failure.
        reason: String,
    },
    /// A glyph run referenced font data with zero bytes.
    #[error("font data is empty")]
    EmptyFontData,
}

/// Convenience `Result` alias for [`VelloError`].
pub type VelloResult<T> = Result<T, VelloError>;
