// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 AppThere Loki contributors

//! Small pure helper functions used by the numbering mapper.

use loki_doc_model::style::list_style::LabelAlignment;

/// Maps a `w:lvlJc` value to [`LabelAlignment`].
pub(super) fn map_lvl_jc(jc: Option<&str>) -> LabelAlignment {
    match jc {
        Some("right" | "end") => LabelAlignment::Right,
        Some("center") => LabelAlignment::Center,
        _ => LabelAlignment::Left,
    }
}

/// Normalize a bullet character, remapping Wingdings/Symbol PUA code points
/// (U+E000..U+F8FF) to the closest standard Unicode equivalent.
///
/// DOCX files produced by Microsoft Word commonly store bullet characters as
/// Private Use Area code points such as U+F0B7 (Wingdings bullet).  These only
/// render correctly when the Wingdings or Symbol font is explicitly applied.
/// Because the layout engine uses a plain `Inline::Str` for the marker (no
/// per-run font override), the character is shaped with the paragraph's default
/// font, which has no glyph for PUA code points → tofu.
///
/// Remapping to standard Unicode lets any font render the bullet correctly.
pub(super) fn normalize_bullet_char(c: char) -> char {
    match c {
        '\u{F06C}' | '\u{F076}' | '\u{F0D8}' => '●', // Wingdings: filled circle → BLACK CIRCLE
        '\u{F06E}' => '○',                           // Wingdings: white circle → WHITE CIRCLE
        '\u{F0FC}' => '■',                           // Wingdings: filled square → BLACK SQUARE
        '\u{F0E8}' => '✓',                           // Wingdings 2: check mark → CHECK MARK
        '\u{F067}' => '–',                           // Wingdings: dash → EN DASH
        // Bullet variants (F0B7/F0A7) and all remaining PUA chars fall back to bullet.
        '\u{E000}'..='\u{F8FF}' => '•',
        _ => c,
    }
}

/// Counts `%N` tokens in a level-text format string (e.g. `"%1.%2."` → 2).
pub(super) fn count_display_levels(lvl_text: &str) -> u8 {
    // A %N token is '%' followed by an ASCII digit.
    let bytes = lvl_text.as_bytes();
    let mut count: u8 = 0;
    let mut i = 0;
    while i + 1 < bytes.len() {
        if bytes[i] == b'%' && bytes[i + 1].is_ascii_digit() {
            count = count.saturating_add(1);
            i += 2;
        } else {
            i += 1;
        }
    }
    count
}
