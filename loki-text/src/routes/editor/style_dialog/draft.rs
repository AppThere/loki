// SPDX-License-Identifier: Apache-2.0

//! The edit buffer behind the paragraph style dialog.
//!
//! # Why this is not [`StyleDraft`](super::super::editor_state::StyleDraft)
//!
//! The inline panel's draft flattens every property to a plain `bool` / `String`
//! and writes `Some(..)` for all of them on Apply — so a style that inherited
//! ten properties owns ten local overrides the moment you press the button, and
//! the provenance lines the design is built around go stale immediately.
//!
//! [`ParaDialogDraft`] keeps the **model's own shape** instead: it holds a
//! [`ParagraphStyle`] whose `Option` fields already mean exactly
//! "set here" vs. "falls through to the parent chain". Editing a control writes
//! `Some(v)`; resetting writes `None`; nothing else is touched. Local-vs-
//! inherited is preserved by construction rather than reconstructed afterwards,
//! which is the whole point of Spec 05 M2 (audit SM-3, "local-only blindness").
//!
//! # Why the numeric fields are buffered
//!
//! A `Points` field cannot hold the intermediate states of typing — `-`, `1.`,
//! `` — so each numeric control keeps a [`String`] buffer alongside the model
//! value. The buffer is the input's value; the model is updated whenever the
//! buffer parses. An **empty** buffer clears the property to `None` (inherit),
//! which is how a user hands a property back to the parent by clearing the box.

use loki_doc_model::loki_primitives::units::Points;
use loki_doc_model::style::ParagraphStyle;

/// Text buffers for the dialog's numeric inputs, so a half-typed value survives
/// a redraw. Field names mirror the [`ParagraphStyle`] property each backs.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub(super) struct ParaNumericBuffers {
    /// `char_props.font_size`, in points.
    pub font_size: String,
    /// `para_props.indent_start`.
    pub indent_start: String,
    /// `para_props.indent_end`.
    pub indent_end: String,
    /// `para_props.indent_first_line`.
    pub indent_first: String,
    /// `para_props.space_before`.
    pub space_before: String,
    /// `para_props.space_after`.
    pub space_after: String,
    /// `para_props.line_height`, as a multiple (`1.35` = 1.35×).
    pub line_height: String,
    /// `para_props.orphan_control`, in lines.
    pub orphan: String,
    /// `para_props.widow_control`, in lines.
    pub widow: String,
    /// Border width, in points.
    pub border_width: String,
    /// `para_props.padding_top`.
    pub padding_top: String,
    /// `para_props.padding_bottom`.
    pub padding_bottom: String,
    /// `para_props.padding_left`.
    pub padding_left: String,
    /// `para_props.padding_right`.
    pub padding_right: String,
    /// Position of the tab stop the user is about to add.
    pub new_tab_stop: String,
}

/// The paragraph style being edited, plus the input buffers.
#[derive(Debug, Clone, PartialEq)]
pub(super) struct ParaDialogDraft {
    /// The style as it will be committed. An `Option::None` on any property
    /// means "not set on this style" — it resolves through the parent chain.
    pub style: ParagraphStyle,
    /// The style as it was when the dialog opened, so Cancel is a discard and
    /// the footer can tell whether anything is staged.
    pub original: ParagraphStyle,
    /// Buffers for the numeric inputs.
    pub buffers: ParaNumericBuffers,
}

impl ParaDialogDraft {
    /// Opens a draft over `style`.
    #[must_use]
    pub fn new(style: ParagraphStyle) -> Self {
        let buffers = ParaNumericBuffers {
            font_size: fmt_points(style.char_props.font_size),
            indent_start: fmt_points(style.para_props.indent_start),
            indent_end: fmt_points(style.para_props.indent_end),
            indent_first: fmt_points(style.para_props.indent_first_line),
            space_before: fmt_spacing(style.para_props.space_before.as_ref()),
            space_after: fmt_spacing(style.para_props.space_after.as_ref()),
            line_height: fmt_line_height(style.para_props.line_height.as_ref()),
            orphan: fmt_u8(style.para_props.orphan_control),
            widow: fmt_u8(style.para_props.widow_control),
            border_width: fmt_points(super::borders::edge_border(&style).map(|b| b.width)),
            padding_top: fmt_points(style.para_props.padding_top),
            padding_bottom: fmt_points(style.para_props.padding_bottom),
            padding_left: fmt_points(style.para_props.padding_left),
            padding_right: fmt_points(style.para_props.padding_right),
            new_tab_stop: String::new(),
        };
        Self {
            original: style.clone(),
            style,
            buffers,
        }
    }

    /// `true` when the user has changed something worth committing. Drives the
    /// footer: Apply is inert on an untouched draft, so pressing it cannot pin
    /// inherited values by accident.
    #[must_use]
    pub fn is_dirty(&self) -> bool {
        self.style != self.original
    }

    /// Clears **every** local override, handing the whole style back to its
    /// parent chain — the footer's "Reset all to inherited".
    ///
    /// Identity (`id`, `display_name`, `parent`, `next_style_id`, the `is_*`
    /// flags, extensions) is not a property and is deliberately kept: this
    /// resets what the style *sets*, not what it *is*.
    pub fn reset_all_to_inherited(&mut self) {
        self.style.para_props = Default::default();
        self.style.char_props = Default::default();
        *self = Self {
            original: self.original.clone(),
            buffers: Self::new(self.style.clone()).buffers,
            style: self.style.clone(),
        };
    }
}

/// Formats an optional point measurement for an input buffer. `None` (inherit)
/// renders as an **empty** box, not `0` — the two are different states, and a
/// zero would claim the style sets a value it does not.
#[must_use]
pub(super) fn fmt_points(pt: Option<Points>) -> String {
    pt.map(|p| trim_float(p.value())).unwrap_or_default()
}

/// Formats an optional line count for an input buffer.
#[must_use]
pub(super) fn fmt_u8(n: Option<u8>) -> String {
    n.map(|v| v.to_string()).unwrap_or_default()
}

/// Formats a `Spacing` for an input buffer. Only `Exact` is editable as a
/// number; any other variant leaves the box empty rather than showing a value
/// the box cannot round-trip.
#[must_use]
pub(super) fn fmt_spacing(s: Option<&loki_doc_model::style::props::para_props::Spacing>) -> String {
    use loki_doc_model::style::props::para_props::Spacing;
    match s {
        Some(Spacing::Exact(pt)) => trim_float(pt.value()),
        _ => String::new(),
    }
}

/// Formats a `LineHeight` multiple for an input buffer.
#[must_use]
pub(super) fn fmt_line_height(
    l: Option<&loki_doc_model::style::props::para_props::LineHeight>,
) -> String {
    use loki_doc_model::style::props::para_props::LineHeight;
    match l {
        Some(LineHeight::Multiple(m)) => trim_float(f64::from(*m)),
        _ => String::new(),
    }
}

/// Renders a float without trailing zeros: `25.4` stays `25.4`, `18.0` becomes
/// `18`. A box that reads `18.0` invites the user to "fix" it and re-type the
/// same number.
#[must_use]
pub(super) fn trim_float(v: f64) -> String {
    let s = format!("{v:.2}");
    let s = s.trim_end_matches('0').trim_end_matches('.');
    if s.is_empty() || s == "-" {
        "0".to_string()
    } else {
        s.to_string()
    }
}

/// Parses an input buffer into an optional measurement.
///
/// Returns `Ok(None)` for an empty buffer — clearing a box hands the property
/// back to the parent — and `Err(())` for text that is not a number, which the
/// caller treats as "leave the model alone while the user is still typing".
/// The three outcomes are distinct on purpose: collapsing "not a number yet"
/// into "unset" would silently drop a value mid-keystroke.
pub(super) fn parse_points(buf: &str) -> Result<Option<Points>, ()> {
    let t = buf.trim();
    if t.is_empty() {
        return Ok(None);
    }
    t.parse::<f64>()
        .map(|v| Some(Points::new(v)))
        .map_err(|_| ())
}

/// Parses an input buffer into an optional line count, clamped to a sane range.
pub(super) fn parse_lines(buf: &str) -> Result<Option<u8>, ()> {
    let t = buf.trim();
    if t.is_empty() {
        return Ok(None);
    }
    t.parse::<u8>()
        .map(|v| Some(v.clamp(1, 10)))
        .map_err(|_| ())
}

/// Parses an input buffer into an optional line-height multiple.
pub(super) fn parse_multiple(buf: &str) -> Result<Option<f32>, ()> {
    let t = buf.trim();
    if t.is_empty() {
        return Ok(None);
    }
    match t.parse::<f32>() {
        Ok(v) if v > 0.0 => Ok(Some(v)),
        // A zero or negative multiple collapses every line onto one baseline;
        // treat it as still-being-typed rather than committing it.
        Ok(_) => Err(()),
        Err(_) => Err(()),
    }
}

#[cfg(test)]
#[path = "draft_tests.rs"]
mod tests;
