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

//! Tracked change (revision) types.
//!
//! TR 29166 §7.2.7 classifies the ODF and OOXML change tracking models as
//! having "difficult" translatability (§8.3.3 classification). This module
//! stores change tracking data opaquely to enable lossless round-trips
//! within a format without attempting cross-format translation.
//!
//! See ADR-0006 for the design rationale.

use crate::content::attr::ExtensionBag;

/// The high-level semantic kind of a tracked change.
///
/// TR 29166 §7.2.7. This classification enables consumers to reason about
/// change tracking without parsing the raw format-specific data.
///
/// Both ODF and OOXML support these four kinds, though with different
/// structural representations. ADR-0006.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[non_exhaustive]
pub enum TrackedChangeKind {
    /// Content was inserted.
    /// ODF: `text:tracked-changes/text:changed-region/text:insertion`.
    /// OOXML: `w:ins`.
    Insert,

    /// Content was deleted.
    /// ODF: `text:tracked-changes/text:changed-region/text:deletion`.
    /// OOXML: `w:del`.
    Delete,

    /// Formatting was changed without inserting or deleting text.
    /// ODF: `text:format-change`. OOXML: `w:rPrChange` / `w:pPrChange`.
    FormatChange,

    /// Content was moved (cut and pasted). OOXML: `w:moveFrom` / `w:moveTo`.
    /// ODF does not have a native move operation; represented as delete+insert.
    Move,
}

/// An opaque tracked change record.
///
/// TR 29166 §7.2.7 classifies the ODF/OOXML change tracking models as
/// having "difficult" translatability (§8.3.3). This type stores the change
/// tracking data from the source format opaquely in `raw`. It is not
/// translated between formats — see ADR-0006.
///
/// The `TrackedChangeRef` inline variant marks where a tracked change begins
/// or ends in the content flow.
///
/// ODF: `text:tracked-changes/text:changed-region`.
/// OOXML: `w:ins` / `w:del` / `w:rPrChange` / `w:pPrChange`.
#[derive(Debug, Clone, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct TrackedChange {
    /// A unique identifier for this change record.
    /// ODF: `text:id`. OOXML: `w:id`.
    pub id: String,

    /// The author who made the change.
    /// ODF: `dc:creator`. OOXML: `w:author`.
    pub author: Option<String>,

    /// The date/time the change was made (UTC).
    /// ODF: `dc:date`. OOXML: `w:date`.
    pub date: Option<chrono::DateTime<chrono::Utc>>,

    /// The semantic kind of the change.
    pub kind: TrackedChangeKind,

    /// Raw format-specific data for lossless round-trip within the same format.
    ///
    /// Cross-format translation of tracked changes is explicitly not supported.
    /// See ADR-0006.
    pub raw: ExtensionBag,
}

impl TrackedChange {
    /// Creates a new [`TrackedChange`] with the given id and kind.
    #[must_use]
    pub fn new(id: impl Into<String>, kind: TrackedChangeKind) -> Self {
        Self {
            id: id.into(),
            author: None,
            date: None,
            kind,
            raw: ExtensionBag::default(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn tracked_change_insert() {
        let tc = TrackedChange::new("r1", TrackedChangeKind::Insert);
        assert_eq!(tc.kind, TrackedChangeKind::Insert);
        assert!(tc.author.is_none());
    }

    #[test]
    fn tracked_change_with_author() {
        let mut tc = TrackedChange::new("r2", TrackedChangeKind::Delete);
        tc.author = Some("Bob".into());
        assert_eq!(tc.author.as_deref(), Some("Bob"));
    }
}
