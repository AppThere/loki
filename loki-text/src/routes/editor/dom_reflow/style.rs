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
pub fn span_css(s: &StyleSpan, families: &super::content::FamilyMap) -> String {
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

/// The OpenType feature list for one resolved run, for the
/// `data-font-features` attribute the vendored `blitz-dom` reads.
///
/// # Why an attribute and not `font-kerning`
///
/// `StyleSpan` carries the document's `w:kern` / `style:letter-kerning` (gap
/// #23), and `para_build` turns anything but `Some(true)` into `"kern" 0`,
/// because Word and LibreOffice default pair kerning **off** while the shaper
/// defaults it on. Left unstated, the DOM path kerned text the canvas path did
/// not — measured (ADR-0017 §5.6) at 0.13 % of a line in ordinary serif prose
/// and **7.7 %** on a kern-heavy string, which is a break moved by words.
///
/// It cannot be said in CSS here: Stylo 0.8 gates both `font-kerning` and
/// `font-feature-settings` to the Gecko engine, so in this build the properties
/// do not exist and a declaration is dropped as unknown — which is worse than
/// nothing, because it reads as a fix. The vendored `blitz-dom` therefore reads
/// the list off `data-font-features` instead; see `docs/patches.md`.
/// `TODO(dom-reflow-kerning)`: emit the CSS property once Stylo exposes it.
#[must_use]
pub fn span_font_features(s: &StyleSpan) -> &'static str {
    if s.kerning == Some(true) {
        "\"kern\" 1"
    } else {
        "\"kern\" 0"
    }
}

/// The CSS declarations for one resolved paragraph.
///
/// # `white-space: pre-wrap`, because the model's characters are the content
///
/// Under CSS's default `normal`, whitespace is collapsed and trimmed — and the
/// text of a paragraph reaches this view as one `<span>` per resolved run, so a
/// space that *ended* one run and a space that *began* the next were both
/// dropped. Measured (ADR-0017 §5.5): `the monospaced words here sits` rendered
/// as `themonospaced words heresits`, and every mixed-run paragraph in the
/// comparison fixture broke differently from the canvas path because of it.
///
/// `pre-wrap` says "these characters, wrapped", which is what the canvas path
/// does: `loki-layout` shapes the model's text as it stands and does not collapse
/// it. `normal` was the wrong mode from the start — it just could not be seen
/// until a paragraph had a run boundary inside it.
///
/// `space_before` / `space_after` become margins here. On the canvas path the
/// flow adds them around the paragraph box rather than inside it — the resolved
/// struct's own docs say they are "handled by the caller, not included in
/// `ParagraphLayout::height`" — and a CSS margin is that same outside space.
#[must_use]
pub fn resolved_para_css(p: &ResolvedParaProps) -> String {
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
        "white-space: pre-wrap; text-align: {align}; margin: {before}pt 0 {after}pt 0; \
         padding-inline-start: {start}pt; padding-inline-end: {end}pt; ",
        before = p.space_before,
        after = p.space_after,
        start = p.indent_start,
        end = p.indent_end,
    );
    // No `text-indent`. See `hanging_row_css` for the measurement: this stack
    // does not implement the property, so emitting it renders a first-line or
    // hanging indent flush — silently, which is worse than not emitting it,
    // because a declaration reads as a fix. `TODO(dom-reflow-text-indent)`: a
    // *positive* first-line indent could be a zero-height inline-block spacer
    // at the head of the runs (an inline box is how an `<img>` reaches this
    // inline flow); a hanging one on a paragraph with no marker cannot be
    // expressed at all, because no box occupies the space it opens up.
    if let Some(bg) = p.background_color {
        css.push_str(&format!("background: {}; ", css_layout_color(bg)));
    }
    css
}

/// The row that carries a list item's marker beside its text.
///
/// # Why a row, and not the hanging indent the model states
///
/// A hanging indent is `text-indent: -Xpt`, and the marker reaches the item's
/// own indent through a **tab stop**. This stack has neither: `parley` 0.6's
/// `TextStyle` carries no indent and no tab stop, and the vendored `blitz-dom`'s
/// `stylo_to_parley` never reads `text-indent` or `tab-size` — so both
/// declarations are dropped as unknown, and a list rendered with them is a list
/// with no hanging indent whose marker is followed by a literal tab character.
///
/// Measured on the list fixture before this existed (ADR-0017 §5.9): every line
/// of every item, first and continuation alike, began at the same x — 57 px,
/// which is the paragraph's `padding-inline-start` and nothing else — and the
/// marker's tab painted as a `.notdef` box.
///
/// So the hanging space becomes a **box**: a flex row indented to
/// `indent_start - indent_hanging`, a marker cell exactly `indent_hanging` wide,
/// and the text in what is left. The text then starts at `indent_start` on every
/// line and every line has the same width — which is exactly what the canvas
/// path's tab stop achieves, so the two break in the same places.
///
/// The paragraph's outside space and its background move here (see
/// [`hanging_body_props`]): margins do not collapse into a flex item, so a
/// `space_before` left on the `<p>` would push its first line below the
/// marker's.
#[must_use]
pub fn hanging_row_css(p: &ResolvedParaProps) -> String {
    let mut css = format!(
        "display: flex; align-items: flex-start; margin: {before}pt 0 {after}pt 0; \
         padding-inline-start: {lead}pt; ",
        before = p.space_before,
        after = p.space_after,
        // Clamped because CSS padding cannot be negative. It only binds when a
        // list's indent is smaller than its own hanging step, which
        // `synthesize_list_item_para` never produces — the marker would then sit
        // left of the column on the canvas path and flush against it here.
        lead = (p.indent_start - p.indent_hanging).max(0.0),
    );
    if let Some(bg) = p.background_color {
        css.push_str(&format!("background: {}; ", css_layout_color(bg)));
    }
    css
}

/// The marker cell of a [`hanging_row_css`] row.
///
/// `min-width` rather than `width`: a marker wider than the hanging step pushes
/// the text right instead of overlapping it. That is *not* what the canvas path
/// does — there the tab advances to the next stop on the default grid, which is
/// a different number — so a marker wider than the step is where the two part.
/// `TODO(dom-reflow-wide-marker)`.
#[must_use]
pub fn hanging_marker_css(p: &ResolvedParaProps) -> String {
    format!("min-width: {}pt; flex-shrink: 0;", p.indent_hanging)
}

/// The text cell of a [`hanging_row_css`] row.
///
/// `min-width: 0` because a flex item's automatic minimum is its min-content
/// width: one unbreakable word longer than the column would otherwise widen the
/// row rather than overflow it, and the item would break at a width the canvas
/// path never sees.
pub const HANGING_BODY_CSS: &str = "flex: 1; min-width: 0;";

/// The paragraph inside a [`hanging_row_css`] row: everything the row does not
/// already carry.
///
/// The indents go to zero because the row states them; the outside space and the
/// background go to zero because the row paints them. Leaving either in both
/// places is the same fact derived twice, and here it would be visible — the
/// indent would apply on top of the row's.
#[must_use]
pub fn hanging_body_props(p: &ResolvedParaProps) -> ResolvedParaProps {
    let mut inner = p.clone();
    inner.indent_start = 0.0;
    inner.indent_hanging = 0.0;
    inner.space_before = 0.0;
    inner.space_after = 0.0;
    inner.background_color = None;
    inner
}

#[cfg(test)]
#[path = "style_tests.rs"]
mod tests;
