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
use loki_doc_model::{MutationError, insert_inline_image, mark_text};
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
    insert_inline_image(loro, focus.paragraph_index, focus.byte_offset, image)?;
    Ok(true)
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
        DocumentPosition::top_level(0, 0, byte_offset)
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

    /// Encodes a `w`×`h` RGBA PNG into bytes for the image-insert tests.
    fn png_bytes(w: u32, h: u32) -> Vec<u8> {
        let img = image::RgbaImage::new(w, h);
        let mut bytes = Vec::new();
        image::DynamicImage::ImageRgba8(img)
            .write_to(&mut Cursor::new(&mut bytes), image::ImageFormat::Png)
            .expect("encode png");
        bytes
    }

    #[test]
    fn image_inline_carries_data_uri_and_intrinsic_size() {
        let inline = image_inline_from_bytes(&png_bytes(2, 3)).expect("png is supported");
        let Inline::Image(attr, _alt, target) = inline else {
            panic!("expected an image");
        };
        assert!(target.url.starts_with("data:image/png;base64,"));
        let cx = attr
            .kv
            .iter()
            .find(|(k, _)| k == "cx_emu")
            .map(|(_, v)| v.as_str());
        let cy = attr
            .kv
            .iter()
            .find(|(k, _)| k == "cy_emu")
            .map(|(_, v)| v.as_str());
        assert_eq!(cx, Some((2 * EMU_PER_PX_96).to_string().as_str()));
        assert_eq!(cy, Some((3 * EMU_PER_PX_96).to_string().as_str()));
    }

    #[test]
    fn non_image_bytes_are_rejected() {
        assert!(image_inline_from_bytes(b"not an image at all").is_none());
    }

    #[test]
    fn insert_image_at_cursor_places_a_discrete_image() {
        let loro = doc_with_text("ab");
        let image = image_inline_from_bytes(&png_bytes(4, 4)).unwrap();
        let applied = insert_image_at_cursor(&loro, &selection(1, 1), &image).unwrap();
        assert!(applied);
        let doc = loki_doc_model::loro_to_document(&loro).unwrap();
        let Block::Para(inlines) = &doc.sections[0].blocks[0] else {
            panic!("para");
        };
        assert_eq!(inlines.len(), 3, "Str, Image, Str: {inlines:?}");
        assert!(matches!(inlines[1], Inline::Image(..)));
    }

    #[test]
    fn insert_image_without_cursor_is_a_noop() {
        let loro = doc_with_text("ab");
        let image = image_inline_from_bytes(&png_bytes(4, 4)).unwrap();
        let no_cursor = CursorState::new();
        assert!(!insert_image_at_cursor(&loro, &no_cursor, &image).unwrap());
    }
}
