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
//! the native [`insert_inline_image`] mapping — no interior editing required.
//!
//! Table and footnote creation follow once the mutation layer can address
//! content nested inside cells and note bodies (tracked separately from the
//! bridge-representation work).

use std::io::Cursor;

use base64::Engine as _;
use image::ImageFormat;
use loki_doc_model::content::attr::NodeAttr;
use loki_doc_model::content::inline::{Inline, LinkTarget};
use loki_doc_model::loro_schema::MARK_LINK_URL;
use loki_doc_model::{MutationError, insert_inline_image_at, mark_text_at};
use loro::{LoroDoc, LoroValue};

use super::editor_formatting::resolve_format_range;
use crate::editing::cursor::CursorState;

/// EMU per CSS pixel at 96 DPI (1 inch = 914 400 EMU = 96 px). Inserted images
/// display at their intrinsic pixel size; layout reads `cx_emu`/`cy_emu`.
const EMU_PER_PX_96: u64 = 9525;

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
    let Some((path, byte_start, byte_end)) = resolve_format_range(loro, cursor) else {
        return Ok(false);
    };
    let trimmed = url.trim();
    let value = if trimmed.is_empty() {
        LoroValue::Null
    } else {
        LoroValue::from(trimmed.to_string())
    };
    mark_text_at(loro, &path, byte_start, byte_end, MARK_LINK_URL, value)?;
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

#[cfg(test)]
#[path = "editor_insert_tests.rs"]
mod tests;
