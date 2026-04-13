// Copyright 2026 AppThere Loki contributors
// SPDX-License-Identifier: Apache-2.0

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
