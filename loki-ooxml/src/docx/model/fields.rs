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

//! Intermediate model for OOXML document fields.
//!
//! OOXML complex fields are assembled from interleaved `w:fldChar` and
//! `w:instrText` elements within `w:r` elements.
//! See ECMA-376 §17.16.

/// An assembled document field (after state-machine processing of
/// `w:fldChar begin → instrText → separate → result → end`).
///
/// ECMA-376 §17.16.17 (complex fields).
#[derive(Debug, Clone)]
pub struct DocxField {
    /// The assembled field instruction string.
    pub instruction: String,
    /// The last-rendered result text (between `separate` and `end`).
    pub current_value: Option<String>,
}

impl DocxField {
    /// Creates a field with the given instruction and no current value.
    #[must_use]
    pub fn new(instruction: impl Into<String>) -> Self {
        Self {
            instruction: instruction.into(),
            current_value: None,
        }
    }

    /// Returns the uppercase field name (first whitespace-delimited word).
    #[must_use]
    pub fn field_name(&self) -> &str {
        self.instruction
            .split_whitespace()
            .next()
            .unwrap_or("")
    }
}

/// State for the field assembly state machine used during paragraph parsing.
///
/// The state machine tracks nesting because fields can be nested.
/// ECMA-376 §17.16.18.
#[derive(Debug, Clone, Default)]
pub struct FieldState {
    /// Accumulated instruction text.
    pub instruction: String,
    /// Accumulated result text.
    pub current_value: Option<String>,
    /// Whether we have passed the `separate` marker.
    pub in_result: bool,
}

impl FieldState {
    /// Creates a new empty field state.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Finalizes this state into a `DocxField`.
    #[must_use]
    pub fn finish(self) -> DocxField {
        DocxField {
            instruction: self.instruction.trim().to_string(),
            current_value: self.current_value,
        }
    }
}
