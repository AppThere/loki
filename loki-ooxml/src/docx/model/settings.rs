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

//! Intermediate model for `word/settings.xml`.
//!
//! Mirrors ECMA-376 §17.15 (document settings).

/// Top-level model for `w:settings` (ECMA-376 §17.15.1.78).
#[derive(Debug, Clone, Default)]
pub struct DocxSettings {
    /// `w:defaultTabStop @w:val` — default tab stop interval in twips.
    pub default_tab_stop: Option<i32>,
    /// `w:evenAndOddHeaders` — whether even/odd headers are different.
    pub even_and_odd_headers: bool,
    /// `w:titlePg` — whether a different first-page header/footer is used.
    pub title_pg: bool,
}
