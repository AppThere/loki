// Copyright 2026 AppThere Loki contributors
// SPDX-License-Identifier: Apache-2.0

//! Document field types — inline dynamic content.
//!
//! TR 29166 §5.2.19 "Document fields" and §4.2 "dynamic content" property.
//! Both ODF and OOXML support fields that are evaluated at render time.
//! This module provides the abstract representation.
//!
//! See ADR-0005 for the design decision on known vs. raw fields and the
//! `current_value` snapshot.

use crate::content::attr::ExtensionBag;

/// The display format for a cross-reference field.
///
/// TR 29166 §5.2.19. ODF `text:reference-format`;
/// OOXML `\* MERGEFORMAT` / format switch on `REF` field.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[non_exhaustive]
pub enum CrossRefFormat {
    /// Displays the paragraph number of the target.
    Number,
    /// Displays the page number of the target.
    Page,
    /// Displays the caption text of the target figure or table.
    Caption,
    /// Displays the label and number (e.g. "Figure 3").
    Label,
    /// Displays the heading text of the target section.
    HeadingText,
}

/// The kind of a document field.
///
/// Known field kinds are given first-class enum variants. Fields whose
/// instruction string cannot be mapped to a known kind are stored as
/// [`FieldKind::Raw`] for lossless round-tripping. ADR-0005.
///
/// TR 29166 §5.2.19 (Document fields feature table).
#[derive(Debug, Clone, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[non_exhaustive]
pub enum FieldKind {
    /// The current page number.
    /// ODF: `text:page-number`. OOXML: `PAGE` field.
    PageNumber,

    /// The total page count.
    /// ODF: `text:page-count`. OOXML: `NUMPAGES` field.
    PageCount,

    /// The current date, optionally with a format string.
    /// ODF: `text:date`. OOXML: `DATE` field.
    Date {
        /// A format string in the source format's date syntax.
        /// `None` uses the application's default date format.
        format: Option<String>,
    },

    /// The current time, optionally with a format string.
    /// ODF: `text:time`. OOXML: `TIME` field.
    Time {
        /// A format string in the source format's time syntax.
        format: Option<String>,
    },

    /// The document title from metadata.
    /// ODF: `text:title`. OOXML: `TITLE` field.
    Title,

    /// The document author from metadata.
    /// ODF: `text:author-name`. OOXML: `AUTHOR` field.
    Author,

    /// The document subject from metadata.
    /// ODF: `text:subject`. OOXML: `SUBJECT` field.
    Subject,

    /// The document file name.
    /// ODF: `text:file-name`. OOXML: `FILENAME` field.
    FileName,

    /// The document word count.
    /// ODF: `text:word-count`. OOXML: `NUMWORDS` field.
    WordCount,

    /// A cross-reference to another element identified by `target`.
    /// ODF: `text:bookmark-ref`. OOXML: `REF` field.
    CrossReference {
        /// The target bookmark or heading identifier.
        target: String,
        /// The format in which to display the reference.
        format: CrossRefFormat,
    },

    /// A field whose instruction string cannot be mapped to a known kind.
    ///
    /// Stored verbatim for lossless round-trips within the same format.
    /// See ADR-0005.
    Raw {
        /// The raw field instruction string.
        instruction: String,
    },
}

/// A document field — inline dynamic content evaluated at render time.
///
/// TR 29166 §5.2.19 "Document fields" and §4.2 "dynamic content" property.
/// ODF: various `text:*` field elements. OOXML: `w:fldChar`/`w:instrText`
/// complex fields, or simple fields via `w:fldSimple`.
///
/// The `current_value` carries the last-rendered snapshot for display
/// when the field cannot be evaluated (e.g. in headless export).
/// See ADR-0005.
#[derive(Debug, Clone, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct Field {
    /// The semantic kind of this field.
    pub kind: FieldKind,

    /// The last-rendered value of the field, if available.
    ///
    /// Format importers populate this from the cached result stored in the
    /// source document. Format exporters may use this value when the field
    /// cannot be re-evaluated.
    pub current_value: Option<String>,

    /// Format-specific extension data.
    pub extensions: ExtensionBag,
}

impl Field {
    /// Creates a [`Field`] with the given kind and no current value.
    #[must_use]
    pub fn new(kind: FieldKind) -> Self {
        Self {
            kind,
            current_value: None,
            extensions: ExtensionBag::default(),
        }
    }

    /// Builder: set the cached current value.
    #[must_use]
    pub fn with_current_value(mut self, value: impl Into<String>) -> Self {
        self.current_value = Some(value.into());
        self
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn field_page_number() {
        let f = Field::new(FieldKind::PageNumber).with_current_value("42");
        assert!(matches!(f.kind, FieldKind::PageNumber));
        assert_eq!(f.current_value.as_deref(), Some("42"));
    }

    #[test]
    fn field_raw_round_trip() {
        let f = Field::new(FieldKind::Raw {
            instruction: "HYPERLINK \"https://example.com\"".into(),
        });
        if let FieldKind::Raw { instruction } = &f.kind {
            assert!(instruction.contains("HYPERLINK"));
        } else {
            panic!("expected Raw variant");
        }
    }
}
