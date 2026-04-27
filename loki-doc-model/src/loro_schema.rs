// Copyright 2026 AppThere Loki contributors
// SPDX-License-Identifier: Apache-2.0

//! Schema constants for mapping Loki documents to Loro CRDT structures.

/// Key for the Document metadata map.
pub const KEY_METADATA: &str = "metadata";

/// Key for the Document style catalog map.
pub const KEY_STYLE_CATALOG: &str = "style_catalog";

/// Key for the Document page size.
pub const KEY_PAGE_SIZE: &str = "page_size";

/// Key for the Document header/footer map.
pub const KEY_HEADER_FOOTER: &str = "header_footer";

/// Key for the Document sections list.
pub const KEY_SECTIONS: &str = "sections";

/// Key for the Document blocks movable list.
pub const KEY_BLOCKS: &str = "blocks";

/// Key for the Block type discriminator.
pub const KEY_TYPE: &str = "type";

/// Key for Block child contents depending on the block type.
pub const KEY_CONTENT: &str = "content";

/// Key for direct CharProps applied on a paragraph Block.
pub const KEY_DIRECT_CHAR_PROPS: &str = "direct_char_props";

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
pub const MARK_LETTER_SPACING: &str = "letter_spacing";
pub const MARK_WORD_SPACING: &str = "word_spacing";
pub const MARK_SCALE: &str = "scale";
pub const MARK_SMALL_CAPS: &str = "small_caps";
pub const MARK_ALL_CAPS: &str = "all_caps";
pub const MARK_SHADOW: &str = "shadow";
pub const MARK_KERNING: &str = "kerning";
pub const MARK_OUTLINE: &str = "outline";

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
pub const PROP_LIST_ID: &str = "list_id";
pub const PROP_LIST_LEVEL: &str = "list_level";
pub const PROP_BIDI: &str = "bidi";
pub const PROP_WIDOW_CONTROL: &str = "widow_control";
pub const PROP_LINE_HEIGHT: &str = "line_height";
pub const PROP_BORDER: &str = "border";
pub const PROP_TAB_STOPS: &str = "tab_stops";
