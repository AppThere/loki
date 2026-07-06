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
//! Each `toggle_*` function reads the mark at the start of the **first**
//! resolved range (the document-order start of the selection).  If the mark
//! is already active there, it is cleared (`LoroValue::Null`) across every
//! range; otherwise it is applied across every range.  This matches
//! Word/LibreOffice toggle behaviour, extended to multi-paragraph selections
//! (plan 4b.2): the state at the selection start decides, and the whole
//! selection is made uniform.
//!
//! # Range resolution
//!
//! [`resolve_format_ranges`](super::editor_format_range::resolve_format_ranges)
//! maps `CursorState` to one `(BlockPath, byte_start, byte_end)` per
//! paragraph: the selection's ranges for single- and multi-paragraph
//! selections within one container, the word at the cursor for a point
//! cursor, and a clamp to the focus paragraph for cross-container
//! selections. See `editor_format_range.rs`.

use loki_doc_model::loro_schema::{
    MARK_BOLD, MARK_ITALIC, MARK_STRIKETHROUGH, MARK_UNDERLINE, MARK_VERTICAL_ALIGN,
};
use loki_doc_model::{BlockPath, MutationError, get_mark_at_path, mark_text_at};
use loro::{LoroDoc, LoroValue};

use super::editor_format_range::resolve_format_ranges;
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

// ── Private helpers ───────────────────────────────────────────────────────────

/// Applies `new_value` for `mark_key` across every resolved range.
fn mark_ranges(
    loro: &LoroDoc,
    ranges: &[(BlockPath, usize, usize)],
    mark_key: &str,
    new_value: &LoroValue,
) -> Result<(), MutationError> {
    for (path, byte_start, byte_end) in ranges {
        mark_text_at(
            loro,
            path,
            *byte_start,
            *byte_end,
            mark_key,
            new_value.clone(),
        )?;
    }
    Ok(())
}

fn toggle_string_mark(
    loro: &LoroDoc,
    cursor: &CursorState,
    mark_key: &str,
    enable_value: &str,
) -> Result<ToggleResult, MutationError> {
    let ranges = resolve_format_ranges(loro, cursor);
    let Some((first_path, first_start, _)) = ranges.first() else {
        return Ok(false);
    };
    let active = matches!(
        get_mark_at_path(loro, first_path, *first_start, mark_key)?,
        Some(LoroValue::String(_))
    );
    let new_value = if active {
        LoroValue::Null
    } else {
        LoroValue::from(enable_value.to_string())
    };
    mark_ranges(loro, &ranges, mark_key, &new_value)?;
    Ok(!active)
}

fn toggle_bool_mark(
    loro: &LoroDoc,
    cursor: &CursorState,
    mark_key: &str,
) -> Result<ToggleResult, MutationError> {
    let ranges = resolve_format_ranges(loro, cursor);
    let Some((first_path, first_start, _)) = ranges.first() else {
        return Ok(false);
    };

    let active = matches!(
        get_mark_at_path(loro, first_path, *first_start, mark_key)?,
        Some(LoroValue::Bool(true))
    );

    let new_value = if active {
        LoroValue::Null
    } else {
        LoroValue::Bool(true)
    };
    mark_ranges(loro, &ranges, mark_key, &new_value)?;
    Ok(!active)
}

fn toggle_vertical_align(
    loro: &LoroDoc,
    cursor: &CursorState,
    target_str: &str,
) -> Result<ToggleResult, MutationError> {
    let ranges = resolve_format_ranges(loro, cursor);
    let Some((first_path, first_start, _)) = ranges.first() else {
        return Ok(false);
    };

    let already_active = matches!(
        get_mark_at_path(loro, first_path, *first_start, MARK_VERTICAL_ALIGN)?,
        Some(LoroValue::String(ref s)) if s.as_str() == target_str
    );

    // COMPAT(loro-schema): VerticalAlign is serialised as Debug repr ("Superscript",
    // "Subscript", "Baseline") matching apply_char_props_marks in loro_bridge/inlines.rs.
    let new_value = if already_active {
        LoroValue::Null
    } else {
        LoroValue::from(target_str.to_string())
    };
    mark_ranges(loro, &ranges, MARK_VERTICAL_ALIGN, &new_value)?;
    Ok(!already_active)
}
