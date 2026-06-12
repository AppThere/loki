// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 AppThere Loki contributors

//! Types and helpers for inline content collection during paragraph flattening.

use loki_doc_model::content::block::Block;
use loki_doc_model::content::field::types::{Field, FieldKind};
use loki_doc_model::content::inline::NoteKind;

/// An inline image collected during paragraph flattening.
///
/// Gathered by [`super::flatten_paragraph`] / `walk_inlines` when an
/// [`loki_doc_model::content::inline::Inline::Image`] is encountered. Passed
/// back to the flow engine so it can emit a [`crate::items::PositionedImage`]
/// after Parley text layout.
#[derive(Debug, Clone)]
pub struct CollectedImage {
    /// Data URI (`"data:image/…;base64,…"`) or external URL.
    pub src: String,
    /// Alt text flattened from the image's alt-text inline children.
    pub alt: Option<String>,
    /// Width in English Metric Units.
    pub cx_emu: u64,
    /// Height in English Metric Units.
    pub cy_emu: u64,
}

/// A footnote or endnote body collected during paragraph flattening.
///
/// Gathered by [`super::flatten_paragraph`] / `walk_inlines` when an
/// [`loki_doc_model::content::inline::Inline::Note`] is encountered. Passed
/// back to the flow engine so it can render the note body at the end of the
/// section.
#[derive(Debug, Clone)]
pub struct CollectedNote {
    /// Sequential note number within the section (1-based).
    pub number: u32,
    /// Whether this is a footnote or an endnote.
    pub kind: NoteKind,
    /// The note body blocks.
    pub blocks: Vec<Block>,
}

/// Return display text for a [`Field`]: use `current_value` if available,
/// otherwise fall back to a kind-specific placeholder.
pub(crate) fn field_display_text(f: &Field) -> String {
    if let Some(ref v) = f.current_value {
        return v.clone();
    }
    match &f.kind {
        FieldKind::PageNumber => "1".to_string(),
        FieldKind::PageCount => "1".to_string(),
        FieldKind::Date { .. } => String::new(),
        FieldKind::Time { .. } => String::new(),
        FieldKind::Title | FieldKind::Author | FieldKind::Subject | FieldKind::FileName => {
            String::new()
        }
        FieldKind::WordCount => String::new(),
        FieldKind::CrossReference { .. } => String::new(),
        FieldKind::Raw { .. } => String::new(),
        _ => String::new(),
    }
}

/// Return the Unicode superscript string for note number `n`.
///
/// Uses Unicode superscript digits (U+00B9, U+00B2, U+00B3, U+2074–U+2079)
/// for n ≤ 9, and `[n]` for larger numbers.
pub(crate) fn superscript_mark(n: u32) -> String {
    match n {
        1 => "\u{00B9}".to_string(),
        2 => "\u{00B2}".to_string(),
        3 => "\u{00B3}".to_string(),
        4 => "\u{2074}".to_string(),
        5 => "\u{2075}".to_string(),
        6 => "\u{2076}".to_string(),
        7 => "\u{2077}".to_string(),
        8 => "\u{2078}".to_string(),
        9 => "\u{2079}".to_string(),
        _ => format!("[{n}]"),
    }
}
