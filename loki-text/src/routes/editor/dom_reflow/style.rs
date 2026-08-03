// SPDX-License-Identifier: Apache-2.0

//! Resolved document properties → CSS declarations, for the DOM reflow view
//! (ADR-0017 §5.1).
//!
//! # Resolved, not direct — and resolved by the *same* code
//!
//! The first version of this module read a paragraph's **direct** properties.
//! That renders most documents unstyled, because formatting lives in named
//! styles: side by side on a screenplay the canvas path set monospaced, centred
//! dialogue and this one set proportional, left-aligned text.
//!
//! It now takes [`loki_layout::para::ResolvedParaProps`] and
//! [`loki_layout::para::StyleSpan`] — the output of `loki_layout`'s own
//! `resolve_para_props` and `flatten_paragraph_with_base`, which is what the
//! canvas path shapes with. Not a second resolver against the same catalog: the
//! *same* resolver, so the two paths cannot disagree about what a style means.
//! A reimplementation would be one more copy of the cascade, and the cascade is
//! exactly the thing whose second copy drifts.
//!
//! # Everything is emitted, because everything is resolved
//!
//! A resolved property is a definite value, so each is written out rather than
//! left to inherit. That is the opposite of the direct-property version, which
//! had to omit unset properties so CSS inheritance could stand in for the
//! model's — and it is better: it does not require Stylo's cascade to agree with
//! ours, because nothing is left for either cascade to decide.
//!
//! # Points, not pixels
//!
//! The model is in points and CSS is authored in `pt`. Blitz resolves `pt` at
//! the CSS-standard 96/72, which is exactly `loki-renderer`'s `PX_TO_PT`;
//! converting by hand would be a second statement of that ratio.

use loki_layout::color::LayoutColor;
use loki_layout::para::{ResolvedParaProps, StyleSpan};

/// A CSS colour for a resolved [`LayoutColor`].
///
/// Alpha is carried through as `rgba()`: a run at less than full opacity is a
/// real thing in the model, and dropping it to `rgb()` would paint a watermark
/// as body text.
#[must_use]
pub(super) fn css_layout_color(c: LayoutColor) -> String {
    let to255 = |v: f32| (v.clamp(0.0, 1.0) * 255.0).round() as u8;
    format!(
        "rgba({}, {}, {}, {})",
        to255(c.r),
        to255(c.g),
        to255(c.b),
        c.a.clamp(0.0, 1.0)
    )
}

/// The CSS declarations for one resolved run.
///
/// `families` maps a requested family to the one `loki-layout` substitutes for
/// it. Emitting the requested name instead would leave Blitz to fall back by its
/// own policy, which is not ours — see [`super::content::FamilyMap`].
#[must_use]
pub(super) fn span_css(s: &StyleSpan, families: &super::content::FamilyMap) -> String {
    let mut css = String::new();
    if let Some(name) = &s.font_name {
        // The substituted family, falling back to the requested one when the map
        // has no entry — which means nothing asked for it during collection, so
        // there is nothing better to say.
        let resolved = families.get(name).unwrap_or(name);
        // Quoted: family names contain spaces, and an unquoted `Liberation Sans`
        // is two keywords rather than one family.
        css.push_str(&format!("font-family: '{}'; ", resolved.replace('\'', "")));
    }
    css.push_str(&format!("font-size: {}pt; ", s.font_size));
    // The **numeric** weight, not `bold`. `StyleSpan` carries both and its own
    // docs say the numeric one supersedes the boolean when a `font_weight`
    // style set it — a semibold run written as `bold` would render at 700.
    css.push_str(&format!("font-weight: {}; ", s.weight));
    css.push_str(if s.italic {
        "font-style: italic; "
    } else {
        "font-style: normal; "
    });
    css.push_str(&format!("color: {}; ", css_layout_color(s.color)));

    // One `text-decoration`, not two: a second declaration would win outright
    // and the first would vanish, so a run that is both underlined and struck
    // would lose its underline.
    let mut decorations = Vec::new();
    if s.underline.is_some() {
        decorations.push("underline");
    }
    if s.strikethrough.is_some() {
        decorations.push("line-through");
    }
    css.push_str(&format!(
        "text-decoration: {}; ",
        if decorations.is_empty() {
            "none".to_string()
        } else {
            decorations.join(" ")
        }
    ));

    if let Some(h) = s.highlight_color {
        css.push_str(&format!("background: {}; ", css_layout_color(h)));
    }
    if let Some(ls) = s.letter_spacing {
        css.push_str(&format!("letter-spacing: {ls}pt; "));
    }
    css
}

/// The CSS declarations for one resolved paragraph.
///
/// `space_before` / `space_after` become margins here. On the canvas path the
/// flow adds them around the paragraph box rather than inside it — the resolved
/// struct's own docs say they are "handled by the caller, not included in
/// `ParagraphLayout::height`" — and a CSS margin is that same outside space.
#[must_use]
pub(super) fn resolved_para_css(p: &ResolvedParaProps) -> String {
    // Matched on the `Debug` form: `parley::Alignment` is not re-exported by
    // `loki-layout` and this crate does not depend on Parley directly. Adding a
    // Parley dependency to the app for one enum would put a shaping crate in the
    // UI layer; naming the variants is the smaller cost, and an unknown one
    // falls through to the reading-order default rather than being invented.
    let align = match format!("{:?}", p.alignment).as_str() {
        "Start" => "start",
        "Middle" | "Center" => "center",
        "End" => "end",
        "Justify" | "Justified" => "justify",
        _ => "start",
    };
    let mut css = format!(
        "text-align: {align}; margin: {before}pt 0 {after}pt 0; \
         padding-inline-start: {start}pt; padding-inline-end: {end}pt; ",
        before = p.space_before,
        after = p.space_after,
        start = p.indent_start,
        end = p.indent_end,
    );
    // Hanging wins over first-line when set, which is the model's own order:
    // both write `text-indent`, and a hanging indent is the negative one.
    if p.indent_hanging > 0.0 {
        css.push_str(&format!("text-indent: -{}pt; ", p.indent_hanging));
    } else if p.indent_first_line != 0.0 {
        css.push_str(&format!("text-indent: {}pt; ", p.indent_first_line));
    }
    if let Some(bg) = p.background_color {
        css.push_str(&format!("background: {}; ", css_layout_color(bg)));
    }
    css
}

#[cfg(test)]
#[path = "style_tests.rs"]
mod tests;
