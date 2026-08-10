// SPDX-License-Identifier: Apache-2.0

//! Reading and writing the selection's direct formatting.
//!
//! # Direct formatting is a fourth level, above every style
//!
//! Span formatting is stored as **Loro marks on a text range**, not as a style.
//! A mark present on the run is *direct formatting*: it sits above the character
//! style, which sits above the paragraph style, which sits above the document
//! default (design note 09). Only the first is removed by "Clear direct
//! formatting", which is why [`SpanMarks`] models presence (`Option`) rather
//! than a resolved value — the absence of a mark is the whole point.

use loki_doc_model::loro_schema::{
    MARK_ALL_CAPS, MARK_BOLD, MARK_COLOR, MARK_FONT_FAMILY, MARK_FONT_SIZE_PT,
    MARK_HIGHLIGHT_COLOR, MARK_ITALIC, MARK_LANGUAGE, MARK_LETTER_SPACING, MARK_SMALL_CAPS,
    MARK_STRIKETHROUGH, MARK_UNDERLINE, MARK_VERTICAL_ALIGN,
};
use loki_doc_model::{BlockPath, MutationError, get_mark_at_path, mark_text_at};
use loro::{LoroDoc, LoroValue};

use super::super::editor_format_range::resolve_format_ranges;
use crate::editing::cursor::CursorState;

/// Every mark key this dialog owns, so "clear direct formatting" cannot forget
/// one it can set. Adding a control without adding its key here would leave a
/// mark the Clear button does not remove.
pub(super) const OWNED_MARKS: [&str; 13] = [
    MARK_BOLD,
    MARK_ITALIC,
    MARK_UNDERLINE,
    MARK_STRIKETHROUGH,
    MARK_SMALL_CAPS,
    MARK_ALL_CAPS,
    MARK_COLOR,
    MARK_HIGHLIGHT_COLOR,
    MARK_FONT_FAMILY,
    MARK_FONT_SIZE_PT,
    MARK_VERTICAL_ALIGN,
    MARK_LETTER_SPACING,
    MARK_LANGUAGE,
];

/// The direct formatting on the selection.
///
/// Every field is `None` when the run carries no such mark — meaning the value
/// falls through to the character style and below. `Some` means the selection
/// overrides it here.
#[derive(Clone, Debug, Default, PartialEq)]
pub(in crate::routes::editor) struct SpanMarks {
    /// `MARK_FONT_FAMILY`.
    pub font_family: Option<String>,
    /// `MARK_FONT_SIZE_PT`, in points.
    pub font_size_pt: Option<f64>,
    /// `MARK_BOLD`.
    pub bold: Option<bool>,
    /// `MARK_ITALIC`.
    pub italic: Option<bool>,
    /// `MARK_UNDERLINE`, as the model's style name.
    pub underline: Option<String>,
    /// `MARK_STRIKETHROUGH`, as the model's style name.
    pub strikethrough: Option<String>,
    /// `MARK_SMALL_CAPS`.
    pub small_caps: Option<bool>,
    /// `MARK_ALL_CAPS`.
    pub all_caps: Option<bool>,
    /// `MARK_COLOR`, as a hex string.
    pub color: Option<String>,
    /// `MARK_HIGHLIGHT_COLOR`, as the model's colour name.
    pub highlight: Option<String>,
    /// `MARK_VERTICAL_ALIGN` — `Superscript` / `Subscript` / `Baseline`.
    pub vertical_align: Option<String>,
    /// `MARK_LETTER_SPACING`, in points.
    pub letter_spacing_pt: Option<f64>,
    /// `MARK_LANGUAGE`, as a BCP-47 tag.
    pub language: Option<String>,
}

impl SpanMarks {
    /// Whether the selection carries any direct formatting at all — what
    /// "Clear direct formatting" acts on, and what dims it when there is none.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        *self == SpanMarks::default()
    }

    /// How many properties are set directly on the run.
    #[must_use]
    pub fn direct_count(&self) -> usize {
        [
            self.font_family.is_some(),
            self.font_size_pt.is_some(),
            self.bold.is_some(),
            self.italic.is_some(),
            self.underline.is_some(),
            self.strikethrough.is_some(),
            self.small_caps.is_some(),
            self.all_caps.is_some(),
            self.color.is_some(),
            self.highlight.is_some(),
            self.vertical_align.is_some(),
            self.letter_spacing_pt.is_some(),
            self.language.is_some(),
        ]
        .into_iter()
        .filter(|set| *set)
        .count()
    }
}

/// Reads the direct formatting at the start of the selection.
///
/// **The start, not the whole range.** A selection spanning a bold word and a
/// plain one has no single answer, and reporting "bold" for it would be a claim
/// the document does not make. The design's header states the selection length
/// so the user can see the range this describes the head of.
#[must_use]
pub(super) fn read_marks(loro: &LoroDoc, cursor: &CursorState) -> SpanMarks {
    let ranges = resolve_format_ranges(loro, cursor);
    let Some((path, start, _)) = ranges.first() else {
        return SpanMarks::default();
    };
    let get = |key: &str| get_mark_at_path(loro, path, *start, key).ok().flatten();
    let string_of = |v: Option<LoroValue>| match v {
        Some(LoroValue::String(s)) => Some(s.to_string()),
        _ => None,
    };
    let bool_of = |v: Option<LoroValue>| match v {
        Some(LoroValue::Bool(b)) => Some(b),
        _ => None,
    };
    let number_of = |v: Option<LoroValue>| match v {
        Some(LoroValue::Double(d)) => Some(d),
        Some(LoroValue::I64(i)) => Some(i as f64),
        _ => None,
    };

    SpanMarks {
        font_family: string_of(get(MARK_FONT_FAMILY)),
        font_size_pt: number_of(get(MARK_FONT_SIZE_PT)),
        bold: bool_of(get(MARK_BOLD)),
        italic: bool_of(get(MARK_ITALIC)),
        underline: string_of(get(MARK_UNDERLINE)),
        strikethrough: string_of(get(MARK_STRIKETHROUGH)),
        small_caps: bool_of(get(MARK_SMALL_CAPS)),
        all_caps: bool_of(get(MARK_ALL_CAPS)),
        color: string_of(get(MARK_COLOR)),
        highlight: string_of(get(MARK_HIGHLIGHT_COLOR)),
        vertical_align: string_of(get(MARK_VERTICAL_ALIGN)),
        letter_spacing_pt: number_of(get(MARK_LETTER_SPACING)),
        language: string_of(get(MARK_LANGUAGE)),
    }
}

/// Writes `next` over the selection, touching only the marks that changed.
///
/// Writing every mark on every Apply would turn "I changed the size" into
/// thirteen direct overrides, which is exactly the flattening the paragraph
/// dialog exists to avoid. A field that went to `None` is written as
/// `LoroValue::Null`, which is how a mark is removed.
pub(super) fn apply_marks(
    loro: &LoroDoc,
    cursor: &CursorState,
    before: &SpanMarks,
    next: &SpanMarks,
) -> Result<(), MutationError> {
    let ranges = resolve_format_ranges(loro, cursor);
    if ranges.is_empty() {
        return Ok(());
    }
    let mut writes: Vec<(&str, LoroValue)> = Vec::new();
    let push_string = |writes: &mut Vec<(&str, LoroValue)>,
                       key: &'static str,
                       old: &Option<String>,
                       new: &Option<String>| {
        if old != new {
            writes.push((key, new.clone().map_or(LoroValue::Null, LoroValue::from)));
        }
    };
    let push_bool = |writes: &mut Vec<(&str, LoroValue)>,
                     key: &'static str,
                     old: &Option<bool>,
                     new: &Option<bool>| {
        if old != new {
            writes.push((key, new.map_or(LoroValue::Null, LoroValue::from)));
        }
    };
    let push_number = |writes: &mut Vec<(&str, LoroValue)>,
                       key: &'static str,
                       old: &Option<f64>,
                       new: &Option<f64>| {
        if old != new {
            writes.push((key, new.map_or(LoroValue::Null, LoroValue::from)));
        }
    };

    push_string(
        &mut writes,
        MARK_FONT_FAMILY,
        &before.font_family,
        &next.font_family,
    );
    push_number(
        &mut writes,
        MARK_FONT_SIZE_PT,
        &before.font_size_pt,
        &next.font_size_pt,
    );
    push_bool(&mut writes, MARK_BOLD, &before.bold, &next.bold);
    push_bool(&mut writes, MARK_ITALIC, &before.italic, &next.italic);
    push_string(
        &mut writes,
        MARK_UNDERLINE,
        &before.underline,
        &next.underline,
    );
    push_string(
        &mut writes,
        MARK_STRIKETHROUGH,
        &before.strikethrough,
        &next.strikethrough,
    );
    push_bool(
        &mut writes,
        MARK_SMALL_CAPS,
        &before.small_caps,
        &next.small_caps,
    );
    push_bool(&mut writes, MARK_ALL_CAPS, &before.all_caps, &next.all_caps);
    push_string(&mut writes, MARK_COLOR, &before.color, &next.color);
    push_string(
        &mut writes,
        MARK_HIGHLIGHT_COLOR,
        &before.highlight,
        &next.highlight,
    );
    push_string(
        &mut writes,
        MARK_VERTICAL_ALIGN,
        &before.vertical_align,
        &next.vertical_align,
    );
    push_number(
        &mut writes,
        MARK_LETTER_SPACING,
        &before.letter_spacing_pt,
        &next.letter_spacing_pt,
    );
    push_string(&mut writes, MARK_LANGUAGE, &before.language, &next.language);

    write_all(loro, &ranges, &writes)
}

/// Removes every mark this dialog owns from the selection — "Clear direct
/// formatting". The styles beneath are untouched, which is the level
/// distinction the button's label promises.
pub(super) fn clear_direct(loro: &LoroDoc, cursor: &CursorState) -> Result<(), MutationError> {
    let ranges = resolve_format_ranges(loro, cursor);
    let writes: Vec<(&str, LoroValue)> = OWNED_MARKS
        .iter()
        .map(|key| (*key, LoroValue::Null))
        .collect();
    write_all(loro, &ranges, &writes)
}

/// Applies each `(key, value)` across every resolved range.
fn write_all(
    loro: &LoroDoc,
    ranges: &[(BlockPath, usize, usize)],
    writes: &[(&str, LoroValue)],
) -> Result<(), MutationError> {
    for (path, start, end) in ranges {
        for (key, value) in writes {
            mark_text_at(loro, path, *start, *end, key, value.clone())?;
        }
    }
    Ok(())
}

#[cfg(test)]
#[path = "marks_tests.rs"]
mod tests;
