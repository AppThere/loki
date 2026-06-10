// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 AppThere Loki contributors

//! Document-wide settings that apply globally rather than per-paragraph.
//!
//! [`DocumentSettings`] holds format-neutral equivalents of OOXML
//! `w:settings` (ECMA-376 §17.15) and ODF document settings.

/// Document-wide settings applying to the document as a whole.
///
/// OOXML: `word/settings.xml` (ECMA-376 §17.15).
/// ODF: `settings.xml` (ODF 1.3 §3.10).
///
/// `None` on [`Document::settings`] means all values use their defaults.
#[derive(Debug, Clone, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct DocumentSettings {
    /// Document-wide default tab stop interval in points.
    ///
    /// Used when no explicit tab stops are defined for a paragraph.
    /// Default: 36.0 pt (0.5 inch) per OOXML spec §17.15.1.25 and
    /// ODF §20.386.
    ///
    /// OOXML: `w:defaultTabStop @w:val` (twips; divide by 20 for points).
    pub default_tab_stop_pt: f32,
}

impl Default for DocumentSettings {
    fn default() -> Self {
        Self {
            default_tab_stop_pt: 36.0,
        }
    }
}
