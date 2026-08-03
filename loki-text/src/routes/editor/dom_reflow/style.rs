// SPDX-License-Identifier: Apache-2.0

//! Document properties → CSS declarations, for the DOM reflow view (ADR-0017).
//!
//! # This is where the ADR's risk lives
//!
//! ADR-0017 §3.2 measured that a paragraph breaks identically through
//! `loki-layout`'s own property resolution and through Blitz's Stylo cascade —
//! *given equivalent inputs*. This module is what makes them equivalent. A
//! property mapped wrongly here does not fail loudly; it reflows the document
//! differently from the canvas path, which is the regression the ADR exists to
//! avoid.
//!
//! So it is pure and unit-tested, separately from the rsx that consumes it.
//!
//! # Points, not pixels
//!
//! The model is in points and CSS is authored in `pt` here rather than
//! converted to `px`. Blitz resolves `pt` at the CSS-standard 96/72, which is
//! exactly `loki-renderer`'s `PX_TO_PT`. Converting by hand would be a second
//! statement of that ratio, and the two would drift.

use loki_doc_model::loki_primitives::color::DocumentColor;
use loki_doc_model::style::props::char_props::CharProps;
use loki_doc_model::style::props::para_props::{ParaProps, ParagraphAlignment};

/// A CSS colour for a [`DocumentColor`], or `None` when it has no direct sRGB
/// form.
///
/// Delegates to [`DocumentColor::to_hex`] rather than re-deriving the
/// conversion: that function already answers exactly this question, including
/// returning `None` for the variants that have no sRGB form, and a second copy
/// of a colour conversion is a second place for it to be wrong.
///
/// Theme colours resolve through a document theme this view does not carry, and
/// CMYK is a print space; both are dropped rather than approximated, because a
/// wrong colour is harder to notice than a missing one. `TODO(dom-reflow-color)`.
#[must_use]
pub(super) fn css_color(c: &DocumentColor) -> Option<String> {
    c.to_hex()
}

/// The CSS declarations for a run's character properties.
///
/// Emits only what is **set**: an absent property must inherit, exactly as it
/// does in the model, and writing a default here would override an ancestor
/// that had a real value.
#[must_use]
pub(super) fn char_css(p: &CharProps) -> String {
    let mut css = String::new();
    if let Some(name) = &p.font_name {
        // Quoted: family names contain spaces, and an unquoted `Liberation Sans`
        // is two keywords rather than one family.
        css.push_str(&format!("font-family: '{}'; ", name.replace('\'', "")));
    }
    if let Some(size) = p.font_size {
        css.push_str(&format!("font-size: {}pt; ", size.value()));
    }
    if p.bold == Some(true) {
        css.push_str("font-weight: bold; ");
    } else if p.bold == Some(false) {
        // Explicitly not bold — a run inside a bold ancestor needs saying.
        css.push_str("font-weight: normal; ");
    }
    if p.italic == Some(true) {
        css.push_str("font-style: italic; ");
    } else if p.italic == Some(false) {
        css.push_str("font-style: normal; ");
    }
    // `text-decoration` is one property with two independent model fields, so
    // the lines are collected and emitted once — two declarations would have the
    // second win and the first vanish.
    let mut decorations = Vec::new();
    if p.underline.is_some() {
        decorations.push("underline");
    }
    if p.strikethrough.is_some() {
        decorations.push("line-through");
    }
    if !decorations.is_empty() {
        css.push_str(&format!("text-decoration: {}; ", decorations.join(" ")));
    }
    if let Some(color) = p.color.as_ref().and_then(css_color) {
        css.push_str(&format!("color: {color}; "));
    }
    css
}

/// The CSS declarations for a block's paragraph properties.
#[must_use]
pub(super) fn para_css(p: &ParaProps) -> String {
    let mut css = String::new();
    if let Some(a) = p.alignment {
        let value = match a {
            ParagraphAlignment::Left => "start",
            ParagraphAlignment::Center => "center",
            ParagraphAlignment::Right => "end",
            ParagraphAlignment::Justify => "justify",
            // A distributed paragraph justifies its last line too; CSS says that
            // with a second property, and `justify` alone is the closest single
            // value. Marked rather than silently equated.
            // TODO(dom-reflow-distribute): `text-align-last: justify`.
            _ => "justify",
        };
        css.push_str(&format!("text-align: {value}; "));
    }
    if let Some(i) = p.indent_start {
        css.push_str(&format!("margin-inline-start: {}pt; ", i.value()));
    }
    if let Some(i) = p.indent_end {
        css.push_str(&format!("margin-inline-end: {}pt; ", i.value()));
    }
    if let Some(i) = p.indent_first_line {
        css.push_str(&format!("text-indent: {}pt; ", i.value()));
    }
    // Hanging indent is a *negative* first-line indent against a start indent,
    // which is how CSS expresses it; the model states it as its own positive
    // quantity. Emitted only when there is no explicit first-line indent, since
    // both write `text-indent` and the model treats first-line as the winner.
    if let (Some(h), None) = (p.indent_hanging, p.indent_first_line) {
        css.push_str(&format!("text-indent: -{}pt; ", h.value()));
    }
    css
}

#[cfg(test)]
#[path = "style_tests.rs"]
mod tests;
