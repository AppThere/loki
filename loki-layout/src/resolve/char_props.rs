// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 AppThere Loki contributors

//! Character property resolution: style chain merging and span construction.

use std::ops::Range;

use loki_doc_model::style::catalog::{StyleCatalog, StyleId};
use loki_doc_model::style::props::char_props::{
    CharProps, StrikethroughStyle as DocStrikethroughStyle, UnderlineStyle as DocUnderlineStyle,
    VerticalAlign as DocVerticalAlign,
};

use crate::para::{FontVariant, StrikethroughStyle, StyleSpan, UnderlineStyle, VerticalAlign};

use super::color::{map_highlight_color, resolve_color};
use super::units::pts_to_f32;

// ── CharProps → StyleSpan mapping ────────────────────────────────────────────
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
//
// Fields SILENTLY DROPPED (out of scope for Group 1):
//   font_name_complex    — complex-script font (BiDi)
//   font_name_east_asian — East Asian font
//   font_size_complex    — complex-script font size
//   background_color     — per-run background (distinct from highlight)
//   outline              — hollow text effect
//   scale                — horizontal text scale (gap #14, P2)
//   kerning              — kerning flag (gap #23, P3)
//   language / language_complex / language_east_asian — locale (gap #30, P3)
//   hyperlink            — URL (gap #11, P1 — handled at Inline level)

/// Walk the character-style parent chain in [`StyleCatalog::character_styles`].
pub(crate) fn resolve_char_style_chain(catalog: &StyleCatalog, id: &StyleId) -> CharProps {
    let Some(style) = catalog.character_styles.get(id) else {
        return CharProps::default();
    };
    let own = style.char_props.clone();
    if let Some(ref parent_id) = style.parent {
        let parent = resolve_char_style_chain(catalog, parent_id);
        own.merged_with_parent(&parent)
    } else {
        own
    }
}

/// Compute the effective [`CharProps`] for a run (3-layer merge).
pub(crate) fn effective_run_char_props(
    run: &loki_doc_model::content::inline::StyledRun,
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

/// Convert a [`CharProps`] snapshot to a [`StyleSpan`] covering `range`.
pub(crate) fn char_props_to_style_span(props: &CharProps, range: Range<usize>) -> StyleSpan {
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

    StyleSpan {
        range,
        font_name: props.font_name.clone(),
        font_size: props.font_size.map(pts_to_f32).unwrap_or(12.0),
        bold: props.bold.unwrap_or(false),
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
        link_url: None, // set by walk_inlines when inside Inline::Link (gap #11)
    }
}

/// Append `text` to `buf` and push a span; no-op for empty strings.
///
/// When `props.all_caps` is set, `text` is uppercased before appending
/// (gap #16 fallback — Parley has no `FontVariantCaps` property).
/// When `active_link_url` is `Some`, the span gets `link_url` set and an
/// auto-underline if not already underlined (gap #11).
#[inline]
pub(crate) fn push_text(
    buf: &mut String,
    spans: &mut Vec<StyleSpan>,
    text: &str,
    props: &CharProps,
    active_link_url: Option<&str>,
) {
    if text.is_empty() {
        return;
    }
    let start = buf.len();
    if props.all_caps == Some(true) {
        buf.push_str(&text.to_uppercase());
    } else {
        buf.push_str(text);
    }
    let mut span = char_props_to_style_span(props, start..buf.len());
    if let Some(url) = active_link_url {
        span.link_url = Some(url.to_string());
        // Auto-underline link text that has no explicit underline decoration.
        if span.underline.is_none() {
            span.underline = Some(UnderlineStyle::Single);
        }
    }
    spans.push(span);
}
