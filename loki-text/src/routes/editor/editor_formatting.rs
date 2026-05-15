// SPDX-License-Identifier: Apache-2.0

//! Character formatting operations for the document editor.
//!
//! All inline formatting actions (bold, italic, underline, strikethrough,
//! superscript, subscript) go through this module.  Both keyboard shortcuts
//! and ribbon buttons call these functions — there is no duplication between
//! the two input paths.
//!
//! # Toggle semantics
//!
//! Each `toggle_*` function reads the mark at the start of the resolved
//! format range.  If the mark is already active, it is cleared (`LoroValue::Null`);
//! otherwise it is applied.  This matches Word/LibreOffice toggle behaviour.
//!
//! # Range resolution
//!
//! [`resolve_format_range`] maps `CursorState` to `(block_index, byte_start, byte_end)`.
//! With an active selection in a single block, the selection is used directly.
//! With a point cursor (no selection), the word at the cursor is expanded.
//! Cross-block selections are clamped to the focus block — a future pass can
//! extend this.
//!
//! # Byte offset coordinate space
//!
//! All byte offsets are UTF-8 byte positions, matching `CursorState` and the
//! `mark_utf8` API used by `mark_text`.

use loki_doc_model::loro_schema::{
    MARK_BOLD, MARK_ITALIC, MARK_STRIKETHROUGH, MARK_UNDERLINE, MARK_VERTICAL_ALIGN,
};
use loki_doc_model::{MutationError, get_block_text, get_mark_at, mark_text};
use loro::{LoroDoc, LoroValue};

use crate::editing::cursor::CursorState;

/// Whether the mark was applied (`true`) or removed (`false`).
pub type ToggleResult = bool;

// ── Public toggle functions ───────────────────────────────────────────────────

/// Toggles bold on the selection or word at the cursor.
pub fn toggle_bold(loro: &LoroDoc, cursor: &CursorState) -> Result<ToggleResult, MutationError> {
    toggle_bool_mark(loro, cursor, MARK_BOLD)
}

/// Toggles italic on the selection or word at the cursor.
pub fn toggle_italic(loro: &LoroDoc, cursor: &CursorState) -> Result<ToggleResult, MutationError> {
    toggle_bool_mark(loro, cursor, MARK_ITALIC)
}

/// Toggles underline on the selection or word at the cursor.
///
/// Writes `"Single"` (matching `UnderlineStyle::Single` Debug repr) rather than
/// a bool, because `read_char_props_from_marks` uses `read_str!` for underline.
pub fn toggle_underline(
    loro: &LoroDoc,
    cursor: &CursorState,
) -> Result<ToggleResult, MutationError> {
    toggle_string_mark(loro, cursor, MARK_UNDERLINE, "Single")
}

/// Toggles strikethrough on the selection or word at the cursor.
///
/// Writes `"Single"` (matching `StrikethroughStyle::Single` Debug repr) rather
/// than a bool, because `read_char_props_from_marks` uses `read_str!` for strikethrough.
pub fn toggle_strikethrough(
    loro: &LoroDoc,
    cursor: &CursorState,
) -> Result<ToggleResult, MutationError> {
    toggle_string_mark(loro, cursor, MARK_STRIKETHROUGH, "Single")
}

/// Toggles superscript on the selection or word at the cursor.
///
/// Removes subscript if active before applying superscript.
pub fn toggle_superscript(
    loro: &LoroDoc,
    cursor: &CursorState,
) -> Result<ToggleResult, MutationError> {
    toggle_vertical_align(loro, cursor, "Superscript")
}

/// Toggles subscript on the selection or word at the cursor.
///
/// Removes superscript if active before applying subscript.
pub fn toggle_subscript(
    loro: &LoroDoc,
    cursor: &CursorState,
) -> Result<ToggleResult, MutationError> {
    toggle_vertical_align(loro, cursor, "Subscript")
}

// ── Range resolution ──────────────────────────────────────────────────────────

/// Resolves the format range from cursor state: `(block_index, byte_start, byte_end)`.
///
/// With a selection spanning a single block, the selection range is returned.
/// With a point cursor (no selection), the word at the cursor is expanded.
/// Cross-block selections are clamped to the focus block.
///
/// Returns `None` when there is no valid cursor position.
///
/// # Limitation
///
/// // TODO(formatting): Extend to multi-block selections by iterating blocks
/// // between anchor and focus and applying the mark to each independently.
pub fn resolve_format_range(loro: &LoroDoc, cursor: &CursorState) -> Option<(usize, usize, usize)> {
    let focus = cursor.focus.as_ref()?;
    let block_index = focus.paragraph_index;

    if cursor.has_selection() {
        let anchor = cursor.anchor.as_ref()?;
        if anchor.paragraph_index == focus.paragraph_index {
            let (start, end) = if anchor.byte_offset <= focus.byte_offset {
                (anchor.byte_offset, focus.byte_offset)
            } else {
                (focus.byte_offset, anchor.byte_offset)
            };
            if start < end {
                return Some((block_index, start, end));
            }
        } else {
            // Cross-block: use focus block from 0 to focus offset as a best-effort.
            if focus.byte_offset > 0 {
                return Some((block_index, 0, focus.byte_offset));
            }
        }
    }

    // No selection — expand to the word at cursor.
    let text = get_block_text(loro, block_index);
    let (word_start, word_end) = word_bounds_at(&text, focus.byte_offset);
    if word_start < word_end {
        Some((block_index, word_start, word_end))
    } else {
        None
    }
}

// ── Private helpers ───────────────────────────────────────────────────────────

fn toggle_string_mark(
    loro: &LoroDoc,
    cursor: &CursorState,
    mark_key: &str,
    enable_value: &str,
) -> Result<ToggleResult, MutationError> {
    let (block_index, byte_start, byte_end) = match resolve_format_range(loro, cursor) {
        Some(r) => r,
        None => return Ok(false),
    };
    let active = matches!(
        get_mark_at(loro, block_index, byte_start, mark_key)?,
        Some(LoroValue::String(_))
    );
    let new_value = if active {
        LoroValue::Null
    } else {
        LoroValue::from(enable_value.to_string())
    };
    mark_text(loro, block_index, byte_start, byte_end, mark_key, new_value)?;
    Ok(!active)
}

fn toggle_bool_mark(
    loro: &LoroDoc,
    cursor: &CursorState,
    mark_key: &str,
) -> Result<ToggleResult, MutationError> {
    let (block_index, byte_start, byte_end) = match resolve_format_range(loro, cursor) {
        Some(r) => r,
        None => return Ok(false),
    };

    let active = matches!(
        get_mark_at(loro, block_index, byte_start, mark_key)?,
        Some(LoroValue::Bool(true))
    );

    let new_value = if active {
        LoroValue::Null
    } else {
        LoroValue::Bool(true)
    };
    mark_text(loro, block_index, byte_start, byte_end, mark_key, new_value)?;
    Ok(!active)
}

fn toggle_vertical_align(
    loro: &LoroDoc,
    cursor: &CursorState,
    target_str: &str,
) -> Result<ToggleResult, MutationError> {
    let (block_index, byte_start, byte_end) = match resolve_format_range(loro, cursor) {
        Some(r) => r,
        None => return Ok(false),
    };

    let already_active = matches!(
        get_mark_at(loro, block_index, byte_start, MARK_VERTICAL_ALIGN)?,
        Some(LoroValue::String(ref s)) if s.as_str() == target_str
    );

    // COMPAT(loro-schema): VerticalAlign is serialised as Debug repr ("Superscript",
    // "Subscript", "Baseline") matching apply_char_props_marks in loro_bridge/inlines.rs.
    let new_value = if already_active {
        LoroValue::Null
    } else {
        LoroValue::from(target_str.to_string())
    };
    mark_text(
        loro,
        block_index,
        byte_start,
        byte_end,
        MARK_VERTICAL_ALIGN,
        new_value,
    )?;
    Ok(!already_active)
}

/// Returns the word boundary around `byte_offset` in `text` as
/// `(word_start_byte, word_end_byte)`.
///
/// A "word character" is alphanumeric or underscore.  If the cursor is
/// on whitespace or punctuation, `word_start == word_end` (empty word).
fn word_bounds_at(text: &str, byte_offset: usize) -> (usize, usize) {
    let clamped = byte_offset.min(text.len());
    let before = &text[..clamped];

    // Walk backward to find the last non-word character, then word starts one
    // character after it.
    let word_start = match before
        .char_indices()
        .rev()
        .find(|(_, c)| !c.is_alphanumeric() && *c != '_')
    {
        Some((i, c)) => i + c.len_utf8(),
        None => 0,
    };

    // Walk forward from clamped to find the first non-word character.
    let after = &text[clamped..];
    let word_end = clamped
        + match after
            .char_indices()
            .find(|(_, c)| !c.is_alphanumeric() && *c != '_')
        {
            Some((i, _)) => i,
            None => after.len(),
        };

    (word_start, word_end)
}
