// Copyright 2024-2026 AppThere
//
// Licensed under the Apache License, Version 2.0 (the "License");
// you may not use this file except in compliance with the License.
// You may obtain a copy of the License at
//
//     http://www.apache.org/licenses/LICENSE-2.0
//
// Unless required by applicable law or agreed to in writing, software
// distributed under the License is distributed on an "AS IS" BASIS,
// WITHOUT WARRANTIES OR CONDITIONS OF ANY KIND, either express or implied.
// See the License for the specific language governing permissions and
// limitations under the License.

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
