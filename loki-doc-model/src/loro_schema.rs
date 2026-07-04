// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 AppThere Loki contributors

//! Schema constants for mapping Loki documents to Loro CRDT structures.

/// Key for the Document metadata map.
pub const KEY_METADATA: &str = "metadata";

/// Key (within the metadata map) for the JSON snapshot of
/// [`crate::meta::DocumentMeta`].
///
/// The full metadata struct — core properties plus the Dublin Core extension —
/// is stored as a single JSON string so it round-trips losslessly through the
/// CRDT (the same approach as opaque block snapshots). [`KEY_META_TITLE`] is
/// also written as a plain mirror for human inspection / legacy readers.
pub const KEY_META_JSON: &str = "meta_json";

/// Key (within the metadata map) for the plain-text document title mirror.
pub const KEY_META_TITLE: &str = "title";

/// Key for the Document style catalog map.
pub const KEY_STYLE_CATALOG: &str = "style_catalog";

/// Key (within the style-catalog map) for the JSON snapshot of the whole
/// [`crate::style::catalog::StyleCatalog`]. A lossless serde snapshot, mirroring
/// [`KEY_META_JSON`], rather than a field-by-field CRDT mapping.
pub const KEY_STYLE_CATALOG_JSON: &str = "catalog_json";

/// Key for the legacy document header/footer map (superseded by KEY_LAYOUT slots).
pub const KEY_HEADER_FOOTER: &str = "header_footer";

/// Key for the Document sections list.
pub const KEY_SECTIONS: &str = "sections";

/// Key for the Document blocks movable list.
pub const KEY_BLOCKS: &str = "blocks";

/// Key for the Document comments map (annotation bodies).
pub const KEY_COMMENTS: &str = "comments";

/// Key for the comments JSON snapshot inside the comments map. Like metadata
/// and the style catalog, comments are stored as a lossless `serde` snapshot.
pub const KEY_COMMENTS_JSON: &str = "comments_json";

/// Key for the Block type discriminator.
pub const KEY_TYPE: &str = "type";

/// Key for Block child contents depending on the block type.
pub const KEY_CONTENT: &str = "content";

/// Key for direct CharProps applied on a paragraph Block.
pub const KEY_DIRECT_CHAR_PROPS: &str = "direct_char_props";

/// Key for the heading level integer stored on a heading Block.
pub const KEY_HEADING_LEVEL: &str = "level";

/// Key for the alignment string ("center", "right", "justify") stored on a
/// heading Block. Derived from `NodeAttr::kv["jc"]` set by the OOXML/ODF
/// mappers. Absent when the heading uses the style's default alignment.
pub const KEY_HEADING_JC: &str = "jc";

/// Key for the source style name stored on a heading Block.
/// The ODF mapper writes the `text:style-name` attribute here so that the
/// layout engine can look up ODF-specific heading properties. Absent for
/// headings that use the hardcoded "Heading1" … "Heading6" fallback.
pub const KEY_HEADING_STYLE: &str = "heading_style";

/// Key for ParaProps applied to a paragraph Block.
pub const KEY_PARA_PROPS: &str = "para_props";

// -----------------------------------------------------------------------------
// Block Types
// -----------------------------------------------------------------------------

pub const BLOCK_TYPE_PARA: &str = "para";
pub const BLOCK_TYPE_HEADING: &str = "heading";
pub const BLOCK_TYPE_BULLET_LIST: &str = "bullet_list";
pub const BLOCK_TYPE_ORDERED_LIST: &str = "ordered_list";
pub const BLOCK_TYPE_TABLE: &str = "table";
pub const BLOCK_TYPE_FIGURE: &str = "figure";
pub const BLOCK_TYPE_CODE_BLOCK: &str = "code_block";
pub const BLOCK_TYPE_HR: &str = "hr";
pub const BLOCK_TYPE_BLOCKQUOTE: &str = "blockquote";
pub const BLOCK_TYPE_STYLED_PARA: &str = "styled_para";
/// Block preserved as an opaque JSON snapshot (see `loro_bridge::opaque`).
/// Used for block types without a native CRDT mapping so that round-trips
/// through Loro are lossless; the snapshot lives under [`KEY_OPAQUE_JSON`].
pub const BLOCK_TYPE_OPAQUE: &str = "opaque";

/// Key for the serialized JSON snapshot of a [`BLOCK_TYPE_OPAQUE`] block.
pub const KEY_OPAQUE_JSON: &str = "opaque_json";

/// Key (within a [`BLOCK_TYPE_TABLE`] block map) for the table's structural
/// skeleton — a `serde`-JSON snapshot of the whole `Table` **with every cell's
/// blocks emptied**. Carries the grid (col specs, widths), section/row layout,
/// spans, cell/row props, borders, caption, and attributes. Cell *content*
/// lives separately under [`KEY_TABLE_CELLS`] as live CRDT containers, so cell
/// text round-trips natively (and concurrent edits to different cells merge)
/// instead of as one opaque blob.
pub const KEY_TABLE_SKELETON: &str = "table_skeleton";

/// Key (within a [`BLOCK_TYPE_TABLE`] block map) for the live cell contents — a
/// movable list with one entry per cell (in head → bodies → foot, row-major
/// order), each entry itself a movable list of the cell's blocks (written via
/// the shared block path). Re-attached to the [`KEY_TABLE_SKELETON`] cells on
/// read by the same traversal order.
pub const KEY_TABLE_CELLS: &str = "table_cells";

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

/// Every inline-object anchor mark key (carried over a single
/// [`OBJECT_REPLACEMENT_CHAR`]). Registered with non-expanding behaviour and
/// shared by the write/read paths. Keep in sync as new inline objects migrate
/// off the opaque-snapshot fallback.
pub const INLINE_OBJECT_MARK_KEYS: &[&str] = &[MARK_IMAGE, MARK_NOTE];

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
];

// -----------------------------------------------------------------------------
// ParaProps Keys
// -----------------------------------------------------------------------------

pub const PROP_ALIGNMENT: &str = "alignment";
pub const PROP_INDENT_LEFT: &str = "indent_left";
pub const PROP_INDENT_RIGHT: &str = "indent_right";
pub const PROP_INDENT_FIRST_LINE: &str = "indent_first_line";
pub const PROP_INDENT_HANGING: &str = "indent_hanging";
pub const PROP_SPACE_BEFORE_PT: &str = "space_before_pt";
pub const PROP_SPACE_AFTER_PT: &str = "space_after_pt";
pub const PROP_KEEP_TOGETHER: &str = "keep_together";
pub const PROP_KEEP_WITH_NEXT: &str = "keep_with_next";
pub const PROP_PAGE_BREAK_AFTER: &str = "page_break_after";
pub const PROP_PAGE_BREAK_BEFORE: &str = "page_break_before";
pub const PROP_LIST_ID: &str = "list_id";
pub const PROP_LIST_LEVEL: &str = "list_level";
pub const PROP_BIDI: &str = "bidi";
pub const PROP_WIDOW_CONTROL: &str = "widow_control";
pub const PROP_ORPHAN_CONTROL: &str = "orphan_control";
pub const PROP_OUTLINE_LEVEL: &str = "outline_level";
pub const PROP_LINE_HEIGHT: &str = "line_height";
pub const PROP_BORDER: &str = "border";
pub const PROP_BORDER_TOP: &str = "border_top";
pub const PROP_BORDER_BOTTOM: &str = "border_bottom";
pub const PROP_BORDER_LEFT: &str = "border_left";
pub const PROP_BORDER_RIGHT: &str = "border_right";
pub const PROP_BORDER_BETWEEN: &str = "border_between";
pub const PROP_PADDING_TOP: &str = "padding_top";
pub const PROP_PADDING_BOTTOM: &str = "padding_bottom";
pub const PROP_PADDING_LEFT: &str = "padding_left";
pub const PROP_PADDING_RIGHT: &str = "padding_right";
pub const PROP_TAB_STOPS: &str = "tab_stops";

// -----------------------------------------------------------------------------
// Section / PageLayout Keys
// -----------------------------------------------------------------------------

/// Key for the section layout map inside a section map.
pub const KEY_LAYOUT: &str = "layout";
/// Sub-map under KEY_LAYOUT for page dimensions.
pub const KEY_PAGE_SIZE: &str = "page_size";
/// Sub-map under KEY_LAYOUT for page margins.
pub const KEY_MARGINS: &str = "margins";
/// Orientation string under KEY_LAYOUT ("Portrait" | "Landscape").
pub const KEY_ORIENTATION: &str = "orientation";
/// Optional sub-map under KEY_LAYOUT for multi-column settings.
pub const KEY_COLUMNS: &str = "columns";

// Header / footer slot keys under KEY_LAYOUT
pub const KEY_HEADER: &str = "header";
pub const KEY_FOOTER: &str = "footer";
pub const KEY_HEADER_FIRST: &str = "header_first";
pub const KEY_FOOTER_FIRST: &str = "footer_first";
pub const KEY_HEADER_EVEN: &str = "header_even";
pub const KEY_FOOTER_EVEN: &str = "footer_even";

// Margin sub-keys (under KEY_MARGINS)
pub const KEY_MARGIN_TOP: &str = "top";
pub const KEY_MARGIN_BOTTOM: &str = "bottom";
pub const KEY_MARGIN_LEFT: &str = "left";
pub const KEY_MARGIN_RIGHT: &str = "right";
pub const KEY_MARGIN_HEADER: &str = "header_dist";
pub const KEY_MARGIN_FOOTER: &str = "footer_dist";
pub const KEY_MARGIN_GUTTER: &str = "gutter";

// Column sub-keys (under KEY_COLUMNS)
pub const KEY_COL_COUNT: &str = "count";
pub const KEY_COL_GAP: &str = "gap";
pub const KEY_COL_SEPARATOR: &str = "separator";
