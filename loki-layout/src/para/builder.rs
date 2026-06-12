// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 AppThere Loki contributors

//! Parley builder helpers: style-pushing, text cleaning, and tab-stop logic.

use parley::{
    FontFamily, FontStyle, FontWeight, LineHeight, RangedBuilder, StyleProperty,
};

use crate::color::LayoutColor;

use super::types::{ResolvedLineHeight, ResolvedParaProps, ResolvedTabStop, StyleSpan};

// ── Tab stop helpers (gap #7) ─────────────────────────────────────────────────

// TODO(tab-default): use Document.settings.default_tab_stop_pt once
// DocumentSettings is threaded through layout_document.
/// Default tab stop interval: 0.5 inch = 36 pt = 720 twips (Word default).
const DEFAULT_TAB_INTERVAL: f32 = 36.0;

/// Return the next tab stop position strictly greater than `x`.
///
/// Searches `stops` (sorted ascending) first; falls back to the default
/// 36 pt grid when no explicit stop is defined beyond `x`.
pub(super) fn next_tab_stop(stops: &[ResolvedTabStop], x: f32, indent_hanging: f32) -> f32 {
    if indent_hanging > 0.0 && x < indent_hanging - 0.5 {
        return indent_hanging;
    }
    if let Some(s) = stops.iter().find(|s| s.position > x + 0.5) {
        s.position
    } else {
        ((x / DEFAULT_TAB_INTERVAL).floor() + 1.0) * DEFAULT_TAB_INTERVAL
    }
}

/// Push paragraph-level defaults and per-span character styles onto `builder`.
///
/// Extracted so the same styles can be applied in both the probe pass (pass 1)
/// and the final pass (pass 2) of the two-pass tab stop expansion.
pub(super) fn push_para_styles(
    builder: &mut RangedBuilder<'_, LayoutColor>,
    para_props: &ResolvedParaProps,
    style_spans: &[StyleSpan],
) {
    builder.push_default(StyleProperty::Brush(LayoutColor::BLACK));
    builder.push_default(StyleProperty::FontSize(12.0));
    match para_props.line_height {
        // MetricsRelative(1.0) is Parley's default — single-spacing from natural
        // font metrics. Correct for OOXML lineRule="auto" w:line="240".
        Some(ResolvedLineHeight::MetricsRelative(m)) => {
            builder.push_default(StyleProperty::LineHeight(LineHeight::MetricsRelative(m)));
        }
        Some(ResolvedLineHeight::Exact(pts)) => {
            builder.push_default(StyleProperty::LineHeight(LineHeight::Absolute(pts)));
        }
        // AtLeast: let Parley use natural metrics (always ≥ author intent for body text).
        // None: natural font metrics — always correct.
        Some(ResolvedLineHeight::AtLeast(_)) | None => {}
    }

    for span in style_spans {
        let r = span.range.clone();
        // For super/subscript (gap #3), reduce font size to 58 %.
        // TODO(super-sub): Parley does not expose baseline-shift.
        let effective_font_size = if span.vertical_align.is_some() {
            span.font_size * 0.58
        } else {
            span.font_size
        };
        builder.push(StyleProperty::FontSize(effective_font_size), r.clone());
        builder.push(StyleProperty::Brush(span.color), r.clone());
        if span.bold {
            builder.push(StyleProperty::FontWeight(FontWeight::BOLD), r.clone());
        }
        if span.italic {
            builder.push(StyleProperty::FontStyle(FontStyle::Italic), r.clone());
        }
        if let Some(ref name) = span.font_name {
            builder.push(
                StyleProperty::FontFamily(FontFamily::named(name.as_str())),
                r.clone(),
            );
        }
        if let Some(lh) = span.line_height {
            builder.push(
                StyleProperty::LineHeight(LineHeight::FontSizeRelative(lh)),
                r.clone(),
            );
        }
        // Underline (gap #17): all variants map to Parley's single underline.
        if span.underline.is_some() {
            builder.push(StyleProperty::Underline(true), r.clone());
        }
        // Strikethrough (gap #18): both variants map to Parley's one variant.
        if span.strikethrough.is_some() {
            builder.push(StyleProperty::Strikethrough(true), r.clone());
        }
        // Letter spacing (gap #13).
        if let Some(ls) = span.letter_spacing {
            builder.push(StyleProperty::LetterSpacing(ls), r.clone());
        }
        // Word spacing (gap #22).
        if let Some(ws) = span.word_spacing {
            builder.push(StyleProperty::WordSpacing(ws), r.clone());
        }
        // Caps variant (gaps #15, #16): SmallCaps stored but not applied (no Parley API).
        // AllCaps: text was already uppercased during flatten_paragraph.
    }
}

/// Strip control characters (except `\t` and `\n`) and BOM from `text`,
/// returning the cleaned string together with byte-index mapping tables.
pub(super) fn clean_text_and_spans(
    text: &str,
    spans: &[StyleSpan],
) -> (String, Vec<StyleSpan>, Vec<usize>, Vec<usize>) {
    let mut clean_text = String::with_capacity(text.len());
    let mut orig_to_clean = vec![0; text.len() + 1];
    let mut clean_to_orig = Vec::with_capacity(text.len() + 1);

    let mut orig_idx = 0;
    let mut clean_idx = 0;

    for c in text.chars() {
        let c_len = c.len_utf8();
        let keep = c == '\t' || c == '\n' || (!c.is_control() && c != '\u{feff}');
        if keep {
            for i in 0..c_len {
                orig_to_clean[orig_idx + i] = clean_idx + i;
                clean_to_orig.push(orig_idx + i);
            }
            clean_text.push(c);
            orig_idx += c_len;
            clean_idx += c_len;
        } else {
            for i in 0..c_len {
                orig_to_clean[orig_idx + i] = clean_idx;
            }
            orig_idx += c_len;
        }
    }
    orig_to_clean[orig_idx] = clean_idx;
    clean_to_orig.push(orig_idx);

    let clean_spans = spans
        .iter()
        .map(|span| {
            let mut clean_span = span.clone();
            let start = orig_to_clean
                .get(span.range.start)
                .copied()
                .unwrap_or(clean_idx);
            let end = orig_to_clean
                .get(span.range.end)
                .copied()
                .unwrap_or(clean_idx);
            clean_span.range = start..end;
            clean_span
        })
        .collect();

    (clean_text, clean_spans, orig_to_clean, clean_to_orig)
}
