// SPDX-License-Identifier: Apache-2.0

//! Insert-tab create operations for the document editor.
//!
//! These build new document content at the cursor against the live Loro CRDT.
//! The first operation is **hyperlink** creation — wrapping the selection (or
//! the word at the cursor) in a [`MARK_LINK_URL`] mark, mirroring the
//! toggle-mark pattern in [`super::editor_formatting`]. It needs no auxiliary
//! model support and no interior editing: the linked text already exists.
//!
//! **Image** insertion picks a file, embeds it as a `data:` URI (the form the
//! renderer decodes), and inserts an `Inline::Image` anchor at the cursor via
//! the native [`insert_inline_image_at`] mapping — no interior editing required.
//!
//! **Table** ([`insert_table_after_cursor`]) inserts an empty grid as a new
//! block after the cursor, and **Footnote** ([`insert_footnote_at_cursor`])
//! anchors a note at the cursor with an empty body — both built as live CRDT
//! containers, so their cells / body are editable via a `BlockPath`.

use std::io::Cursor;

use base64::Engine as _;
use image::ImageFormat;
use loki_doc_model::content::attr::NodeAttr;
use loki_doc_model::content::block::Block;
use loki_doc_model::content::inline::{Inline, LinkTarget, NoteKind};
use loki_doc_model::content::table::core::Table;
use loki_doc_model::loro_schema::MARK_LINK_URL;
use loki_doc_model::{
    MutationError, insert_block_after, insert_inline_image_at, insert_inline_note_at, mark_text_at,
};
use loro::{LoroDoc, LoroValue};

use super::editor_format_range::resolve_format_ranges;
use crate::editing::cursor::CursorState;

/// EMU per CSS pixel at 96 DPI (1 inch = 914 400 EMU = 96 px). Inserted images
/// display at their intrinsic pixel size; layout reads `cx_emu`/`cy_emu`.
const EMU_PER_PX_96: u64 = 9525;

/// Applies (or clears) a hyperlink over the selection or the word at the cursor.
///
/// An empty/whitespace-only `url` clears any existing link; otherwise every
/// resolved range is marked with [`MARK_LINK_URL`]. A multi-paragraph selection
/// resolves to one range per paragraph (like the character-formatting toggles),
/// so the whole selection is linked — not just its first paragraph. Returns
/// `true` when a link was applied, `false` when it was cleared or there was no
/// resolvable range (e.g. the cursor sits on whitespace with no selection).
pub fn set_hyperlink(
    loro: &LoroDoc,
    cursor: &CursorState,
    url: &str,
) -> Result<bool, MutationError> {
    let ranges = resolve_format_ranges(loro, cursor);
    if ranges.is_empty() {
        return Ok(false);
    }
    let trimmed = url.trim();
    let value = if trimmed.is_empty() {
        LoroValue::Null
    } else {
        LoroValue::from(trimmed.to_string())
    };
    for (path, byte_start, byte_end) in &ranges {
        mark_text_at(
            loro,
            path,
            *byte_start,
            *byte_end,
            MARK_LINK_URL,
            value.clone(),
        )?;
    }
    Ok(!trimmed.is_empty())
}

/// Builds an [`Inline::Image`] from raw image `bytes`, embedding them as a
/// `data:` URI sized to the image's intrinsic pixel dimensions.
///
/// Returns `None` when the bytes are not a supported raster image (PNG, JPEG,
/// GIF, WebP, BMP) — the format is detected from the bytes, not a file name, so
/// a mislabelled extension still works (or is cleanly rejected).
pub fn image_inline_from_bytes(bytes: &[u8]) -> Option<Inline> {
    let reader = image::ImageReader::new(Cursor::new(bytes))
        .with_guessed_format()
        .ok()?;
    let mime = match reader.format()? {
        ImageFormat::Png => "image/png",
        ImageFormat::Jpeg => "image/jpeg",
        ImageFormat::Gif => "image/gif",
        ImageFormat::WebP => "image/webp",
        ImageFormat::Bmp => "image/bmp",
        _ => return None,
    };
    let (w, h) = reader.into_dimensions().ok()?;
    let uri = format!(
        "data:{mime};base64,{}",
        base64::engine::general_purpose::STANDARD.encode(bytes)
    );
    let mut attr = NodeAttr::default();
    attr.kv.push((
        "cx_emu".to_string(),
        (u64::from(w) * EMU_PER_PX_96).to_string(),
    ));
    attr.kv.push((
        "cy_emu".to_string(),
        (u64::from(h) * EMU_PER_PX_96).to_string(),
    ));
    Some(Inline::Image(attr, Vec::new(), LinkTarget::new(uri)))
}

/// Inserts `image` (an [`Inline::Image`]) at the cursor's focus position.
///
/// Returns `true` when inserted, `false` when there is no placed cursor.
pub fn insert_image_at_cursor(
    loro: &LoroDoc,
    cursor: &CursorState,
    image: &Inline,
) -> Result<bool, MutationError> {
    let Some(focus) = cursor.focus.as_ref() else {
        return Ok(false);
    };
    insert_inline_image_at(loro, &focus.block_path(), focus.byte_offset, image)?;
    Ok(true)
}

/// Default dimensions for the Insert → Table control.
const DEFAULT_TABLE_ROWS: usize = 2;
const DEFAULT_TABLE_COLS: usize = 2;

/// Inserts an empty footnote at the cursor's focus position.
///
/// The note anchors at the cursor and its body is a single empty paragraph the
/// user can then edit (its body is a live container reachable via a
/// `BlockPath`). Returns `true` when inserted, `false` when there is no cursor.
pub fn insert_footnote_at_cursor(
    loro: &LoroDoc,
    cursor: &CursorState,
) -> Result<bool, MutationError> {
    let Some(focus) = cursor.focus.as_ref() else {
        return Ok(false);
    };
    insert_inline_note_at(
        loro,
        &focus.block_path(),
        focus.byte_offset,
        &NoteKind::Footnote,
        &[Block::Para(Vec::new())],
    )?;
    Ok(true)
}

/// Inserts a default empty table immediately after the cursor's (root) block.
///
/// Returns the new block's global index, or `None` when there is no cursor. The
/// table's cells are empty paragraphs the user edits by clicking into them.
pub fn insert_table_after_cursor(
    loro: &LoroDoc,
    cursor: &CursorState,
) -> Result<Option<usize>, MutationError> {
    let Some(focus) = cursor.focus.as_ref() else {
        return Ok(None);
    };
    let table = Block::Table(Box::new(Table::grid(
        DEFAULT_TABLE_ROWS,
        DEFAULT_TABLE_COLS,
    )));
    let new_index = insert_block_after(loro, focus.paragraph_index, &table)?;
    Ok(Some(new_index))
}

#[cfg(test)]
#[path = "editor_insert_tests.rs"]
mod tests;
