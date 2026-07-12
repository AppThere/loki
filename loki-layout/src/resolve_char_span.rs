// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 AppThere Loki contributors

//! Character-property resolution (split from `resolve.rs` for the 300-line
//! ceiling): walks the character-style parent chain, computes a run's
//! effective [`CharProps`] via the 3-layer merge, and converts a `CharProps`
//! snapshot into a renderer-agnostic [`StyleSpan`]. `effective_run_char_props`
//! and `char_props_to_style_span` are re-exported from `resolve.rs`.

use std::ops::Range;

use loki_doc_model::content::inline::StyledRun;
use loki_doc_model::style::catalog::{StyleCatalog, StyleId};
use loki_doc_model::style::props::char_props::{
    CharProps, StrikethroughStyle as DocStrikethroughStyle, UnderlineStyle as DocUnderlineStyle,
    VerticalAlign as DocVerticalAlign,
};

use super::{pts_to_f32, resolve_color};
use crate::color::LayoutColor;
use crate::para::{FontVariant, StrikethroughStyle, StyleSpan, UnderlineStyle, VerticalAlign};

/// Maximum number of parent links followed when resolving a character-style
/// chain. Guards against cyclic `parent` references in corrupt documents
/// (e.g. A.parent = B, B.parent = A). When the cap is exceeded, inheritance
/// stops — the chain is treated as if it ended at a root style.
const MAX_STYLE_CHAIN_DEPTH: usize = 32;

/// Walk the character-style parent chain in [`StyleCatalog::character_styles`].
pub(super) fn resolve_char_style_chain(catalog: &StyleCatalog, id: &StyleId) -> CharProps {
    let Some(style) = catalog.character_styles.get(id) else {
        return CharProps::default();
    };
    let mut resolved = style.char_props.clone();
    let mut parent_id = style.parent.as_ref();
    for _ in 0..MAX_STYLE_CHAIN_DEPTH {
        let Some(parent) = parent_id.and_then(|pid| catalog.character_styles.get(pid)) else {
            break;
        };
        resolved = resolved.merged_with_parent(&parent.char_props);
        parent_id = parent.parent.as_ref();
    }
    resolved
}

/// Compute the effective [`CharProps`] for a run (3-layer merge).
pub(super) fn effective_run_char_props(
    run: &StyledRun,
    catalog: &StyleCatalog,
    parent: &CharProps,
) -> CharProps {
    let mut props = parent.clone();
    if let Some(ref id) = run.style_id {
        props = resolve_char_style_chain(catalog, id).merged_with_parent(&props);
    }
    if let Some(ref direct) = run.direct_props {
        props = direct.as_ref().clone().merged_with_parent(&props);
    }
    props
}

// ── char_props_to_style_span ──────────────────────────────────────────────────
//
// Audit of CharProps → StyleSpan mapping (Group 1 gaps, 2026-04-20):
//
// Fields CURRENTLY MAPPED (pre-session):
//   font_name          → StyleSpan.font_name
//   font_size          → StyleSpan.font_size (default 12.0)
//   bold               → StyleSpan.bold
//   italic             → StyleSpan.italic
//   color              → StyleSpan.color
//   underline          → StyleSpan.underline (was bool; now Option<UnderlineStyle>)
//   strikethrough      → StyleSpan.strikethrough (was bool; now Option<StrikethroughStyle>)
//
// Fields ADDED in this session (Group 1 gaps):
//   vertical_align     → StyleSpan.vertical_align       (gap #3,  P0)
//   highlight_color    → StyleSpan.highlight_color      (gap #10, P1)
//   letter_spacing     → StyleSpan.letter_spacing       (gap #13, P2)
//   small_caps/all_caps→ StyleSpan.font_variant         (gap #15/#16, P2)
//   underline variant  → StyleSpan.underline Option     (gap #17, P2)
//   strikethrough var  → StyleSpan.strikethrough Option (gap #18, P2)
//   word_spacing       → StyleSpan.word_spacing         (gap #22, P3)
//   shadow             → StyleSpan.shadow               (gap #24, P3)
//   scale              → StyleSpan.scale                (gap #14, P2)
//
//   kerning            → StyleSpan.kerning (gap #23, P3; shaper toggle, default OFF)
//   language           → StyleSpan.language (gap #30, P3; spell-check routing)
//
// Fields SILENTLY DROPPED (out of scope for Group 1):
//   font_name_complex    — complex-script font (BiDi)
//   font_name_east_asian — East Asian font
//   font_size_complex    — complex-script font size
//   background_color     — per-run background (distinct from highlight)
//   outline              — hollow text effect
//   language_complex / language_east_asian — script-specific locale variants
//   hyperlink            — URL (gap #11, P1 — handled at Inline level)

/// Convert a [`CharProps`] snapshot to a [`StyleSpan`] covering `range`.
pub(super) fn char_props_to_style_span(props: &CharProps, range: Range<usize>) -> StyleSpan {
    // Superscript / subscript (gap #3): map to layout VerticalAlign.
    let vertical_align = match props.vertical_align {
        Some(DocVerticalAlign::Superscript) => Some(VerticalAlign::Superscript),
        Some(DocVerticalAlign::Subscript) => Some(VerticalAlign::Subscript),
        _ => None,
    };

    // Highlight colour (gap #10): convert named palette to LayoutColor.
    // Fall back to background_color (w:shd @fill on runs) when no named
    // highlight is set — both serve the same visual role.
    let highlight_color = map_highlight_color(props.highlight_color).or_else(|| {
        props
            .background_color
            .as_ref()
            .map(|c| resolve_color(Some(c)))
    });

    // Underline (gap #17): preserve variant (Parley renders all as single).
    let underline = match props.underline {
        Some(DocUnderlineStyle::Single) => Some(UnderlineStyle::Single),
        Some(DocUnderlineStyle::Double) => Some(UnderlineStyle::Double),
        Some(DocUnderlineStyle::Dotted) => Some(UnderlineStyle::Dotted),
        Some(DocUnderlineStyle::Dash) => Some(UnderlineStyle::Dash),
        Some(DocUnderlineStyle::Wave) => Some(UnderlineStyle::Wave),
        Some(DocUnderlineStyle::Thick) => Some(UnderlineStyle::Thick),
        None => None,
        // Non-exhaustive guard: future doc-model variants default to Single.
        _ => Some(UnderlineStyle::Single),
    };

    // Strikethrough (gap #18): preserve variant.
    let strikethrough = match props.strikethrough {
        Some(DocStrikethroughStyle::Single) => Some(StrikethroughStyle::Single),
        Some(DocStrikethroughStyle::Double) => Some(StrikethroughStyle::Double),
        None => None,
        _ => Some(StrikethroughStyle::Single),
    };

    // Caps variant (gaps #15, #16): small_caps takes precedence over all_caps.
    let font_variant = if props.small_caps == Some(true) {
        Some(FontVariant::SmallCaps)
    } else if props.all_caps == Some(true) {
        Some(FontVariant::AllCaps)
    } else {
        None
    };

    let bold = props.bold.unwrap_or(false);
    let mut span = StyleSpan {
        range,
        font_name: props.font_name.clone(),
        font_size: props.font_size.map(pts_to_f32).unwrap_or(12.0),
        bold,
        // Explicit numeric weight wins; otherwise derive from the bold flag.
        weight: props.font_weight.unwrap_or(if bold { 700 } else { 400 }),
        italic: props.italic.unwrap_or(false),
        color: resolve_color(props.color.as_ref()),
        underline,
        strikethrough,
        line_height: None,
        vertical_align,
        highlight_color,
        letter_spacing: props.letter_spacing.map(pts_to_f32), // gap #13
        font_variant,
        word_spacing: props.word_spacing.map(pts_to_f32), // gap #22
        shadow: props.shadow.unwrap_or(false),            // gap #24
        kerning: props.kerning,                           // gap #23
        link_url: None, // set by walk_inlines when inside Inline::Link (gap #11)
        math: None,     // set by walk_inlines for Inline::Math placeholders
        // Horizontal text scale (gap #14): only forward a non-trivial, positive
        // factor so the common 100 % case stays on the fast (unscaled) path.
        scale: props
            .scale
            .filter(|&s| s > 0.0 && (s - 1.0).abs() > f32::EPSILON),
        // Manual baseline rise (gap: w:position): forward only a non-zero shift.
        baseline_shift: props
            .baseline_shift
            .map(pts_to_f32)
            .filter(|&s| s.abs() > f32::EPSILON),
        // Language tag (gap #30): routes per-run spell checking.
        language: props.language.as_ref().map(|t| t.as_str().into()),
    };
    crate::revision_style::apply(&mut span, props);
    span
}

/// Convert a [`HighlightColor`] palette entry to a [`LayoutColor`].
///
/// Returns `None` for [`HighlightColor::None`] (explicit highlight removal).
fn map_highlight_color(
    hc: Option<loki_doc_model::style::props::char_props::HighlightColor>,
) -> Option<LayoutColor> {
    use loki_doc_model::style::props::char_props::HighlightColor::*;
    match hc? {
        Yellow => Some(LayoutColor::new(1.000, 1.000, 0.000, 1.0)),
        Green => Some(LayoutColor::new(0.000, 1.000, 0.000, 1.0)),
        Cyan => Some(LayoutColor::new(0.000, 1.000, 1.000, 1.0)),
        Magenta => Some(LayoutColor::new(1.000, 0.000, 1.000, 1.0)),
        Blue => Some(LayoutColor::new(0.000, 0.000, 1.000, 1.0)),
        Red => Some(LayoutColor::new(1.000, 0.000, 0.000, 1.0)),
        DarkBlue => Some(LayoutColor::new(0.000, 0.000, 0.502, 1.0)),
        DarkCyan => Some(LayoutColor::new(0.000, 0.502, 0.502, 1.0)),
        DarkGreen => Some(LayoutColor::new(0.000, 0.502, 0.000, 1.0)),
        DarkMagenta => Some(LayoutColor::new(0.502, 0.000, 0.502, 1.0)),
        DarkRed => Some(LayoutColor::new(0.502, 0.000, 0.000, 1.0)),
        DarkYellow => Some(LayoutColor::new(0.502, 0.502, 0.000, 1.0)),
        DarkGray => Some(LayoutColor::new(0.502, 0.502, 0.502, 1.0)),
        LightGray => Some(LayoutColor::new(0.753, 0.753, 0.753, 1.0)),
        Black => Some(LayoutColor::BLACK),
        White => Some(LayoutColor::WHITE),
        None => Option::None,
        _ => Option::None,
    }
}
