// Copyright 2026 AppThere Loki contributors
// SPDX-License-Identifier: Apache-2.0

//! CRDT mutation helpers for in-editor text insertion and deletion.
//!
//! All functions operate on the live [`LoroDoc`] that is the authoritative
//! editing state (ADR-0006).  After each mutation the caller must call
//! [`crate::loro_bridge::loro_to_document`] to re-derive the read-only
//! [`Document`] snapshot for layout and rendering.
//!
//! # Container navigation
//!
//! The Loro schema for text paragraphs is:
//! ```text
//! sections (LoroList)
//!   └─ [section_index] (LoroMap)
//!         └─ "blocks" (LoroMovableList)
//!               └─ [block_index] (LoroMap)
//!                     └─ "content" (LoroText)
//! ```
//!
//! # Byte vs. char positions
//!
//! [`DocumentPosition::byte_offset`] is a byte offset into the paragraph's
//! UTF-8 text.  The Loro API uses Unicode *char* (codepoint) positions.
//! [`byte_offset_to_char_index`] converts between the two representations.

use loro::LoroDoc;

use crate::loro_schema::*;

/// Errors that can occur during document text mutation.
#[derive(Debug, thiserror::Error)]
pub enum MutationError {
    /// An error returned by the underlying Loro library.
    #[error("Loro error: {0}")]
    Loro(String),
    /// The requested section index does not exist in the document.
    #[error("Section index {0} out of bounds")]
    SectionOutOfBounds(usize),
    /// The requested block index does not exist in the section.
    #[error("Block index {0} out of bounds")]
    BlockOutOfBounds(usize),
    /// The block at the given index has no text content container.
    #[error("Block {0} has no text content container")]
    NoContent(usize),
    /// The byte offset is not a valid UTF-8 char boundary in the paragraph text.
    #[error("Byte offset {offset} is not on a char boundary in text of {len} bytes")]
    InvalidByteOffset {
        /// The invalid byte offset.
        offset: usize,
        /// Length of the text in bytes.
        len: usize,
    },
}

impl From<loro::LoroError> for MutationError {
    fn from(e: loro::LoroError) -> Self {
        MutationError::Loro(e.to_string())
    }
}

// ── Private navigation helper ─────────────────────────────────────────────────

/// Navigate the LoroDoc hierarchy and return a handle to the `LoroText`
/// content container for the given `(section_index, block_index)` pair.
///
/// The returned handle has interior mutability: calling `insert`/`delete` on
/// it mutates the underlying CRDT container atomically.
fn get_loro_text(
    loro: &LoroDoc,
    section_index: usize,
    block_index: usize,
) -> Result<loro::LoroText, MutationError> {
    // sections[section_index]
    let sections = loro.get_list(KEY_SECTIONS);
    let sec_map = sections
        .get(section_index)
        .and_then(|v| v.into_container().ok())
        .and_then(|c| c.into_map().ok())
        .ok_or(MutationError::SectionOutOfBounds(section_index))?;

    // sections[section_index].blocks[block_index]
    let blocks = sec_map
        .get(KEY_BLOCKS)
        .and_then(|v| v.into_container().ok())
        .and_then(|c| c.into_movable_list().ok())
        .ok_or(MutationError::BlockOutOfBounds(block_index))?;

    let block_map = blocks
        .get(block_index)
        .and_then(|v| v.into_container().ok())
        .and_then(|c| c.into_map().ok())
        .ok_or(MutationError::BlockOutOfBounds(block_index))?;

    // sections[section_index].blocks[block_index].content (LoroText)
    let text = block_map
        .get(KEY_CONTENT)
        .and_then(|v| v.into_container().ok())
        .and_then(|c| c.into_text().ok())
        .ok_or(MutationError::NoContent(block_index))?;

    Ok(text)
}

/// Convert a UTF-8 byte offset into a Unicode char (codepoint) index.
///
/// Returns an error when `byte_offset` is not on a char boundary or exceeds
/// the text length.
fn byte_offset_to_char_index(text: &str, byte_offset: usize) -> Result<usize, MutationError> {
    if byte_offset > text.len() {
        return Err(MutationError::InvalidByteOffset { offset: byte_offset, len: text.len() });
    }
    if byte_offset == 0 {
        return Ok(0);
    }
    if !text.is_char_boundary(byte_offset) {
        return Err(MutationError::InvalidByteOffset { offset: byte_offset, len: text.len() });
    }
    Ok(text[..byte_offset].chars().count())
}

// ── Public mutation API ───────────────────────────────────────────────────────

/// Insert `text` at `byte_offset` in the content of `block_index` within
/// `section_index`.
///
/// `byte_offset` is a UTF-8 byte offset as carried by
/// [`DocumentPosition::byte_offset`].  It is converted to a Unicode char
/// position before calling the Loro API.
pub fn insert_text(
    loro: &LoroDoc,
    section_index: usize,
    block_index: usize,
    byte_offset: usize,
    text: &str,
) -> Result<(), MutationError> {
    let loro_text = get_loro_text(loro, section_index, block_index)?;
    let current = loro_text.to_string();
    let char_index = byte_offset_to_char_index(&current, byte_offset)?;
    loro_text.insert(char_index, text)?;
    Ok(())
}

/// Delete `byte_len` bytes starting at `byte_offset` from the content of
/// `block_index` within `section_index`.
///
/// Both `byte_offset` and `byte_offset + byte_len` must be on UTF-8 char
/// boundaries.  The byte length is converted to a Unicode char count before
/// calling the Loro API.
pub fn delete_text(
    loro: &LoroDoc,
    section_index: usize,
    block_index: usize,
    byte_offset: usize,
    byte_len: usize,
) -> Result<(), MutationError> {
    if byte_len == 0 {
        return Ok(());
    }
    let loro_text = get_loro_text(loro, section_index, block_index)?;
    let current = loro_text.to_string();
    let char_start = byte_offset_to_char_index(&current, byte_offset)?;
    let char_end = byte_offset_to_char_index(&current, byte_offset + byte_len)?;
    let char_count = char_end - char_start;
    if char_count > 0 {
        loro_text.delete(char_start, char_count)?;
    }
    Ok(())
}

/// Return the plain-text content of the block at `(section_index, block_index)`
/// as a `String`.
///
/// Used by keyboard handlers to compute grapheme boundaries before deletion.
pub fn get_paragraph_text(
    loro: &LoroDoc,
    section_index: usize,
    block_index: usize,
) -> Result<String, MutationError> {
    let loro_text = get_loro_text(loro, section_index, block_index)?;
    Ok(loro_text.to_string())
}

// ── Tests ─────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use crate::content::attr::NodeAttr;
    use crate::content::block::{Block, StyledParagraph};
    use crate::content::inline::Inline;
    use crate::document::Document;
    use crate::layout::section::Section;
    use crate::loro_bridge::document_to_loro;

    /// Build a single-section, single-block LoroDoc containing `text`.
    fn make_loro(text: &str) -> loro::LoroDoc {
        let para = Block::StyledPara(StyledParagraph {
            style_id: None,
            direct_para_props: None,
            direct_char_props: None,
            inlines: vec![Inline::Str(text.into())],
            attr: NodeAttr::default(),
        });
        let mut section = Section::new();
        section.blocks.push(para);
        let doc = Document {
            meta: Default::default(),
            styles: Default::default(),
            sections: vec![section],
            source: None,
        };
        document_to_loro(&doc).expect("document_to_loro")
    }

    #[test]
    fn get_paragraph_text_returns_content() {
        let loro = make_loro("Hello world");
        let text = get_paragraph_text(&loro, 0, 0).unwrap();
        assert_eq!(text, "Hello world");
    }

    #[test]
    fn insert_at_byte_offset_0_prepends() {
        let loro = make_loro("Hello");
        insert_text(&loro, 0, 0, 0, "X").unwrap();
        assert_eq!(get_paragraph_text(&loro, 0, 0).unwrap(), "XHello");
    }

    #[test]
    fn insert_at_end_appends() {
        let loro = make_loro("Hello");
        insert_text(&loro, 0, 0, 5, " world").unwrap();
        assert_eq!(get_paragraph_text(&loro, 0, 0).unwrap(), "Hello world");
    }

    #[test]
    fn insert_in_middle_of_ascii() {
        let loro = make_loro("Hello");
        insert_text(&loro, 0, 0, 3, "Z").unwrap();
        assert_eq!(get_paragraph_text(&loro, 0, 0).unwrap(), "HelZlo");
    }

    #[test]
    fn delete_first_char() {
        let loro = make_loro("Hello");
        delete_text(&loro, 0, 0, 0, 1).unwrap();
        assert_eq!(get_paragraph_text(&loro, 0, 0).unwrap(), "ello");
    }

    #[test]
    fn delete_range() {
        let loro = make_loro("Hello");
        delete_text(&loro, 0, 0, 1, 2).unwrap();
        assert_eq!(get_paragraph_text(&loro, 0, 0).unwrap(), "Hlo");
    }

    #[test]
    fn delete_zero_len_is_noop() {
        let loro = make_loro("Hello");
        delete_text(&loro, 0, 0, 2, 0).unwrap();
        assert_eq!(get_paragraph_text(&loro, 0, 0).unwrap(), "Hello");
    }

    #[test]
    fn section_out_of_bounds_returns_error() {
        let loro = make_loro("Hello");
        assert!(insert_text(&loro, 99, 0, 0, "X").is_err());
        assert!(delete_text(&loro, 99, 0, 0, 1).is_err());
        assert!(get_paragraph_text(&loro, 99, 0).is_err());
    }

    #[test]
    fn block_out_of_bounds_returns_error() {
        let loro = make_loro("Hello");
        assert!(insert_text(&loro, 0, 99, 0, "X").is_err());
        assert!(delete_text(&loro, 0, 99, 0, 1).is_err());
        assert!(get_paragraph_text(&loro, 0, 99).is_err());
    }

    #[test]
    fn insert_multibyte_unicode() {
        let loro = make_loro("Hi");
        // Insert a 2-byte character "é" (U+00E9) at the end.
        insert_text(&loro, 0, 0, 2, "\u{00E9}").unwrap();
        let text = get_paragraph_text(&loro, 0, 0).unwrap();
        assert_eq!(text, "Hi\u{00E9}");
    }

    #[test]
    fn delete_multibyte_unicode() {
        // "é" is 2 bytes in UTF-8.
        let loro = make_loro("H\u{00E9}llo");
        // Delete "é" (2 bytes starting at offset 1).
        delete_text(&loro, 0, 0, 1, 2).unwrap();
        assert_eq!(get_paragraph_text(&loro, 0, 0).unwrap(), "Hllo");
    }

    #[test]
    fn byte_offset_to_char_index_ascii() {
        assert_eq!(byte_offset_to_char_index("Hello", 0).unwrap(), 0);
        assert_eq!(byte_offset_to_char_index("Hello", 3).unwrap(), 3);
        assert_eq!(byte_offset_to_char_index("Hello", 5).unwrap(), 5);
    }

    #[test]
    fn byte_offset_to_char_index_multibyte() {
        // "é" is 2 bytes; "Hello" starts at byte 0, "é" at 0, "l" at 2.
        let s = "\u{00E9}llo"; // é=2 bytes, l=1, l=1, o=1 → total 5 bytes
        assert_eq!(byte_offset_to_char_index(s, 0).unwrap(), 0);
        assert_eq!(byte_offset_to_char_index(s, 2).unwrap(), 1); // after "é"
        assert_eq!(byte_offset_to_char_index(s, 3).unwrap(), 2); // after "él"
    }

    #[test]
    fn byte_offset_not_on_boundary_returns_error() {
        // "é" occupies bytes 0..2; offset 1 is in the middle.
        let s = "\u{00E9}llo";
        assert!(byte_offset_to_char_index(s, 1).is_err());
    }
}
