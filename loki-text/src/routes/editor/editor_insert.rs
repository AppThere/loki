// SPDX-License-Identifier: Apache-2.0

//! Insert-tab create operations for the document editor.
//!
//! These build new document content at the cursor against the live Loro CRDT.
//! The first operation is **hyperlink** creation — wrapping the selection (or
//! the word at the cursor) in a [`MARK_LINK_URL`] mark, mirroring the
//! toggle-mark pattern in [`super::editor_formatting`]. It needs no auxiliary
//! model support and no interior editing: the linked text already exists.
//!
//! Image, table, and footnote creation follow in later increments — image and
//! table once a media/block-insert path lands; table and footnote bodies are
//! only useful once the mutation layer can address content nested inside cells
//! and note bodies (tracked separately from the bridge-representation work).

use loki_doc_model::loro_schema::MARK_LINK_URL;
use loki_doc_model::{MutationError, mark_text};
use loro::{LoroDoc, LoroValue};

use super::editor_formatting::resolve_format_range;
use crate::editing::cursor::CursorState;

/// Applies (or clears) a hyperlink over the selection or the word at the cursor.
///
/// An empty/whitespace-only `url` clears any existing link; otherwise the
/// resolved range is marked with [`MARK_LINK_URL`]. Returns `true` when a link
/// was applied, `false` when it was cleared or there was no resolvable range
/// (e.g. the cursor sits on whitespace with no selection).
pub fn set_hyperlink(
    loro: &LoroDoc,
    cursor: &CursorState,
    url: &str,
) -> Result<bool, MutationError> {
    let Some((block_index, byte_start, byte_end)) = resolve_format_range(loro, cursor) else {
        return Ok(false);
    };
    let trimmed = url.trim();
    let value = if trimmed.is_empty() {
        LoroValue::Null
    } else {
        LoroValue::from(trimmed.to_string())
    };
    mark_text(
        loro,
        block_index,
        byte_start,
        byte_end,
        MARK_LINK_URL,
        value,
    )?;
    Ok(!trimmed.is_empty())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::editing::cursor::DocumentPosition;
    use loki_doc_model::content::block::Block;
    use loki_doc_model::content::inline::Inline;
    use loki_doc_model::document::Document;
    use loki_doc_model::{document_to_loro, get_mark_at};

    fn doc_with_text(text: &str) -> LoroDoc {
        let mut doc = Document::new();
        doc.sections[0].blocks = vec![Block::Para(vec![Inline::Str(text.into())])];
        document_to_loro(&doc).expect("document_to_loro")
    }

    fn pos(byte_offset: usize) -> DocumentPosition {
        DocumentPosition {
            page_index: 0,
            paragraph_index: 0,
            byte_offset,
        }
    }

    fn selection(start: usize, end: usize) -> CursorState {
        CursorState {
            loro_cursor: None,
            anchor: Some(pos(start)),
            focus: Some(pos(end)),
            document_generation: 0,
        }
    }

    fn link_at(loro: &LoroDoc, byte: usize) -> Option<String> {
        match get_mark_at(loro, 0, byte, MARK_LINK_URL).expect("get_mark_at") {
            Some(LoroValue::String(s)) => Some(s.to_string()),
            _ => None,
        }
    }

    #[test]
    fn applies_link_over_selection() {
        let loro = doc_with_text("hello world");
        let applied = set_hyperlink(&loro, &selection(0, 5), "https://example.com").unwrap();
        assert!(applied);
        assert_eq!(link_at(&loro, 0).as_deref(), Some("https://example.com"));
        // Outside the selection there is no link.
        assert_eq!(link_at(&loro, 7), None);
    }

    #[test]
    fn point_cursor_links_word_at_cursor() {
        let loro = doc_with_text("hello world");
        // A point cursor inside "world" links the whole word.
        let applied = set_hyperlink(&loro, &selection(8, 8), "https://w.example").unwrap();
        assert!(applied);
        assert_eq!(link_at(&loro, 8).as_deref(), Some("https://w.example"));
        assert_eq!(link_at(&loro, 0), None);
    }

    #[test]
    fn empty_url_clears_link_and_reports_false() {
        let loro = doc_with_text("hello world");
        assert!(set_hyperlink(&loro, &selection(0, 5), "https://example.com").unwrap());
        let applied = set_hyperlink(&loro, &selection(0, 5), "   ").unwrap();
        assert!(!applied, "blank url clears the link");
        assert_eq!(link_at(&loro, 0), None);
    }

    #[test]
    fn url_is_trimmed() {
        let loro = doc_with_text("hello world");
        set_hyperlink(&loro, &selection(0, 5), "  https://trim.example  ").unwrap();
        assert_eq!(link_at(&loro, 0).as_deref(), Some("https://trim.example"));
    }
}
