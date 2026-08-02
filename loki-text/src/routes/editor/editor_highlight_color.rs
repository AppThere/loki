// SPDX-License-Identifier: Apache-2.0

//! Text highlight colour for the ribbon (Spec 08 T5.3).
//!
//! # One kind of pick, two places it can land
//!
//! The picker hands this a `#RRGGBB` — from a swatch, the hex field, the recent
//! list or the document group, all the same shape. Where it goes depends only on
//! the colour:
//!
//! - **Exactly one of the sixteen `w:highlight` colours** →
//!   [`MARK_HIGHLIGHT_COLOR`], carrying the variant name. This is Word's
//!   Highlight control, and the overwhelmingly common case.
//! - **Anything else** → [`MARK_BACKGROUND_COLOR`], carrying the hex. This is
//!   `w:shd @fill` on export and character shading in the model.
//!
//! The match is on the **resolved colour**, never on how the reader picked it —
//! `HighlightColor::from_hex` is the one place that decides, so a typed
//! `#FFFF00` and a clicked Yellow swatch are the same document (Spec 08 T5.3,
//! spike S0.5 §3).
//!
//! # What a custom highlight looks like in Word, stated rather than discovered
//!
//! `w:highlight` is a fixed enumeration — that is the format, not our
//! limitation — so an arbitrary colour has to travel as `w:shd`, and Word shows
//! `w:shd` under **Shading** rather than under Highlight. A reader who applies
//! `#C0392B` here, exports to DOCX and opens it in Word will see the colour
//! painted correctly and find it in the Shading control. That is the correct
//! trade and is user-visible behaviour, not a defect. ODF has no such limit
//! (`fo:background-color` takes any value), so the ODT path is unaffected.
//!
//! # Exactly one of the two marks is ever set
//!
//! Both marks reach the same painted colour — the layout falls back to
//! `background_color` when there is no named highlight — so leaving a stale one
//! behind would make the *other* one silently win, and which one won would
//! depend on the order the reader picked colours in. Every apply clears both and
//! sets at most one.

use loki_doc_model::loro_schema::{MARK_BACKGROUND_COLOR, MARK_HIGHLIGHT_COLOR};
use loki_doc_model::style::props::char_props::HighlightColor;
use loki_doc_model::{MutationError, get_mark_at_path, mark_text_at};
use loro::{LoroDoc, LoroValue};

use super::editor_format_range::resolve_format_ranges;
use crate::editing::cursor::CursorState;

/// Where a picked highlight colour is stored.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub(super) enum HighlightRoute {
    /// A named `w:highlight` colour, stored as its variant name.
    Named(HighlightColor),
    /// Any other colour, stored as character shading (`w:shd @fill`).
    Shading,
    /// No highlight — both marks cleared.
    Cleared,
}

/// The route a picked colour takes.
///
/// Separate from [`apply_highlight`] so the decision is testable without a
/// `LoroDoc`, and so there is one statement of it to point at.
#[must_use]
pub(super) fn route_for(hex: Option<&str>) -> HighlightRoute {
    match hex {
        None => HighlightRoute::Cleared,
        Some(h) => match HighlightColor::from_hex(h) {
            Some(named) => HighlightRoute::Named(named),
            None => HighlightRoute::Shading,
        },
    }
}

/// Applies `hex` (`Some("#RRGGBB")`) as the highlight across the selection, or
/// removes any direct highlight (`None`).
pub(super) fn apply_highlight(
    loro: &LoroDoc,
    cursor: &CursorState,
    hex: Option<&str>,
) -> Result<(), MutationError> {
    let (named, shading) = match route_for(hex) {
        // The variant name, which is what `decode_highlight_color` reads back.
        HighlightRoute::Named(v) => (LoroValue::from(format!("{v:?}")), LoroValue::Null),
        // `unwrap_or_default` cannot fire: `Shading` is only reached with a
        // `Some`. Written without a panic rather than with one the lint would
        // have to be told about.
        HighlightRoute::Shading => (
            LoroValue::Null,
            LoroValue::from(hex.unwrap_or_default().to_string()),
        ),
        HighlightRoute::Cleared => (LoroValue::Null, LoroValue::Null),
    };
    for (path, start, end) in &resolve_format_ranges(loro, cursor) {
        // Both, always — see the module note on why a stale mark would decide
        // the colour.
        mark_text_at(
            loro,
            path,
            *start,
            *end,
            MARK_HIGHLIGHT_COLOR,
            named.clone(),
        )?;
        mark_text_at(
            loro,
            path,
            *start,
            *end,
            MARK_BACKGROUND_COLOR,
            shading.clone(),
        )?;
    }
    Ok(())
}

/// The direct highlight **colour** at the caret's first resolved range as
/// `#RRGGBB`, or `None` when there is none. Drives the active swatch and the
/// ribbon trigger's indicator.
///
/// Returns a hex whichever mark carries it, because the picker's active-swatch
/// comparison is against swatch values, and those are hexes. Returning a variant
/// name for one route and a hex for the other is what made the document-colour
/// group show transparent swatches before T5.3.
pub(super) fn current_highlight(loro: &LoroDoc, cursor: &CursorState) -> Option<String> {
    let ranges = resolve_format_ranges(loro, cursor);
    let (path, start, _) = ranges.first()?;
    // Named first, matching the paint order: the layout uses `background_color`
    // only when there is no named highlight, so a reader looking at a named
    // highlight must see the named one reported.
    if let Ok(Some(LoroValue::String(s))) =
        get_mark_at_path(loro, path, *start, MARK_HIGHLIGHT_COLOR)
        && let Some(hex) = named_hex(&s)
    {
        return Some(hex.to_string());
    }
    match get_mark_at_path(loro, path, *start, MARK_BACKGROUND_COLOR) {
        Ok(Some(LoroValue::String(s))) => Some(s.to_string()),
        _ => None,
    }
}

/// A stored variant name back to its `#RRGGBB`.
///
/// `None` for `"None"` (an explicit "no highlight", which has no colour) and for
/// anything unrecognised — which is what a pre-T5.3 document containing a hex in
/// the highlight mark looks like, and it should read as "no highlight" rather
/// than as a colour nothing can paint.
fn named_hex(stored: &str) -> Option<&'static str> {
    loki_doc_model::style::props::HIGHLIGHT_RGB
        .iter()
        .find(|(v, _)| format!("{v:?}") == stored)
        .map(|(_, hex)| *hex)
}

#[cfg(test)]
#[path = "editor_highlight_color_tests.rs"]
mod tests;
