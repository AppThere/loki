// Copyright 2026 AppThere Loki contributors
// SPDX-License-Identifier: Apache-2.0

//! Application-level error types for `loki-text`.
//!
//! [`AppError`] is the top-level error enum.  Components convert library
//! errors into `AppError` before surfacing them through UI signals so that
//! the presentation layer never has to deal with foreign error types directly.

use thiserror::Error;

/// Top-level application error.
///
/// Variants wrap errors from the crates that `loki-text` depends on so that
/// they can be handled uniformly via the `?` operator and displayed in the UI.
///
/// Not yet wired up in this scaffold; will be used once document loading
/// is implemented.
#[allow(dead_code)]
#[derive(Debug, Error)]
pub enum AppError {
    /// The platform file picker failed to open or returned an error.
    #[error("file picker error: {0}")]
    FilePicker(#[from] loki_file_access::PickerError),

    /// A serialised [`loki_file_access::FileAccessToken`] could not be parsed.
    #[error("invalid file token: {0}")]
    TokenParse(#[from] loki_file_access::TokenParseError),
}

/// Convenience `Result` alias using [`AppError`].
#[allow(dead_code)]
pub type AppResult<T> = Result<T, AppError>;
