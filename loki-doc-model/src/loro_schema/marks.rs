// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 AppThere Loki contributors

//! Text-formatting mark keys and inline-object anchor constants.
//!
//! Split from the parent `loro_schema` module to keep files under the
//! 300-line ceiling.

// -----------------------------------------------------------------------------
// CharProps Mark Keys
// -----------------------------------------------------------------------------

pub const MARK_BOLD: &str = "bold";
pub const MARK_ITALIC: &str = "italic";
pub const MARK_UNDERLINE: &str = "underline";
pub const MARK_STRIKETHROUGH: &str = "strikethrough";
pub const MARK_COLOR: &str = "color";
pub const MARK_HIGHLIGHT_COLOR: &str = "highlight_color";
pub const MARK_FONT_FAMILY: &str = "font_family";
pub const MARK_FONT_SIZE_PT: &str = "font_size_pt";
pub const MARK_VERTICAL_ALIGN: &str = "vertical_align";
pub const MARK_LINK_URL: &str = "link_url";
pub const MARK_LANGUAGE: &str = "language";
pub const MARK_LANGUAGE_COMPLEX: &str = "language_complex";
pub const MARK_LANGUAGE_EAST_ASIAN: &str = "language_east_asian";
pub const MARK_LETTER_SPACING: &str = "letter_spacing";
pub const MARK_WORD_SPACING: &str = "word_spacing";
pub const MARK_SCALE: &str = "scale";
pub const MARK_SMALL_CAPS: &str = "small_caps";
pub const MARK_ALL_CAPS: &str = "all_caps";
pub const MARK_SHADOW: &str = "shadow";
pub const MARK_KERNING: &str = "kerning";
/// Named character style (`StyledRun::style_id`) carried as a mark so that
/// run-level style references survive Loro round-trips.
pub const MARK_CHAR_STYLE_ID: &str = "char_style_id";
pub const MARK_OUTLINE: &str = "outline";
/// `Inline::Quoted`'s quote style over the quoted range: "Single" | "Double".
pub const MARK_QUOTE_TYPE: &str = "quote_type";
/// `Inline::Span`'s `NodeAttr` as a `serde`-JSON snapshot over the span range.
pub const MARK_SPAN_ATTR: &str = "span_attr";
/// A live tracked-change (revision) mark over the run range — the packed
/// `RevisionMark` string (`style::props::revision::encode`). Review tab / 4a.2.
pub const MARK_REVISION: &str = "revision";

// -----------------------------------------------------------------------------
// Inline objects (anchored by a placeholder char + a data-bearing mark)
// -----------------------------------------------------------------------------

/// Object-replacement character (U+FFFC) — the in-text anchor for an inline
/// object whose structured data is carried by a mark over the single anchor
/// position. The anchor occupies one Unicode scalar so the object is a
/// discrete, positioned, deletable element in the flat text stream.
pub const OBJECT_REPLACEMENT_CHAR: char = '\u{FFFC}';

/// String form of [`OBJECT_REPLACEMENT_CHAR`] for `LoroText::insert`.
pub const OBJECT_REPLACEMENT_STR: &str = "\u{FFFC}";

/// Inline image data: a `serde`-JSON snapshot of the `Inline::Image`, carried
/// as a mark over a single [`OBJECT_REPLACEMENT_CHAR`] anchor so that
/// image-bearing paragraphs round-trip *natively* (the image is a live,
/// positioned inline object) instead of as opaque block snapshots.
pub const MARK_IMAGE: &str = "image";

/// Inline footnote/endnote reference: carried as a mark over a single
/// [`OBJECT_REPLACEMENT_CHAR`] anchor. The value is a `serde`-JSON
/// `(NoteKind, usize)` pair — the note's kind and the index of its **body** in
/// the block's [`KEY_NOTES`] container. The `usize` index also makes each note's
/// mark unique, so adjacent notes do not merge into one rich-text delta span.
///
/// The body itself is a **live CRDT container** (not a JSON blob in the mark),
/// so footnote text is editable and mergeable like a table cell's.
pub const MARK_NOTE: &str = "note";

/// Key (within a paragraph-like block's map) for the block's note bodies — a
/// movable list with one entry per [`MARK_NOTE`] anchor in the block, each a
/// movable list of the note body's blocks (written via the shared block path).
/// Indexed by the `usize` carried in each anchor's [`MARK_NOTE`] mark.
pub const KEY_NOTES: &str = "notes";

/// Comment anchor (`Inline::Comment`): the whole inline as a `serde`-JSON
/// snapshot carried as a mark over a single [`OBJECT_REPLACEMENT_CHAR`], so
/// comment range markers survive CRDT round-trips as positioned anchors.
pub const MARK_COMMENT: &str = "comment";

/// Bookmark marker (`Inline::Bookmark`): the whole inline as a `serde`-JSON
/// snapshot carried as a mark over a single [`OBJECT_REPLACEMENT_CHAR`].
pub const MARK_BOOKMARK: &str = "bookmark";

/// Every inline-object anchor mark key (carried over a single
/// [`OBJECT_REPLACEMENT_CHAR`]). Registered with non-expanding behaviour and
/// shared by the write/read paths. Keep in sync as new inline objects migrate
/// off the opaque-snapshot fallback.
pub const INLINE_OBJECT_MARK_KEYS: &[&str] = &[MARK_IMAGE, MARK_NOTE, MARK_COMMENT, MARK_BOOKMARK];

/// Every character-level mark key (formatting that lives on a text range).
///
/// Single source of truth shared by `document_to_loro` (which registers each
/// key's `expand` behaviour) and `replace_text` (which resets an inserted
/// range's full formatting). Keep this in sync with the read/write paths.
pub const CHAR_MARK_KEYS: &[&str] = &[
    MARK_BOLD,
    MARK_ITALIC,
    MARK_UNDERLINE,
    MARK_STRIKETHROUGH,
    MARK_COLOR,
    MARK_HIGHLIGHT_COLOR,
    MARK_FONT_FAMILY,
    MARK_FONT_SIZE_PT,
    MARK_VERTICAL_ALIGN,
    MARK_LINK_URL,
    MARK_LANGUAGE,
    MARK_LANGUAGE_COMPLEX,
    MARK_LANGUAGE_EAST_ASIAN,
    MARK_LETTER_SPACING,
    MARK_WORD_SPACING,
    MARK_SCALE,
    MARK_SMALL_CAPS,
    MARK_ALL_CAPS,
    MARK_SHADOW,
    MARK_KERNING,
    MARK_OUTLINE,
    MARK_CHAR_STYLE_ID,
    MARK_QUOTE_TYPE,
    MARK_SPAN_ATTR,
    MARK_REVISION,
];
