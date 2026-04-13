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
