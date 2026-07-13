// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 AppThere Loki contributors

//! Parley builder preparation helpers (split from `para.rs` for the 300-line
//! ceiling): pushing paragraph-level defaults + per-span character styles, and
//! reserving inline boxes for typeset math placeholders. Shared by the probe
//! and final passes of `layout_paragraph_uncached` and by `para_band`.

use parley::{
    FontFamily, FontFeatures, FontStyle, FontWeight, InlineBox, InlineBoxKind, LineHeight,
    OverflowWrap, RangedBuilder, StyleProperty,
};

use super::{MATH_ID_BASE, ResolvedLineHeight, ResolvedParaProps, StyleSpan};
use crate::color::LayoutColor;

/// Pushes one Parley inline box per typeset math placeholder, sized to the
/// equation's intrinsic box so the surrounding text flows around it. Ids are
/// offset by [`MATH_ID_BASE`] so the post-layout pass can recognise them.
///
/// The box height is the equation's **ascent** only: Parley aligns an inline
/// box's bottom to the text baseline (counting its whole height as ascent), so
/// reserving just the ascent lands the box top at `baseline − ascent`. Drawing
/// the equation there puts its baseline on the text baseline; the descent then
/// hangs below into the line's descent region, exactly like inline text.
pub(super) fn push_math_inline_boxes(
    builder: &mut RangedBuilder<'_, LayoutColor>,
    math_boxes: &[(usize, crate::math::MathRender)],
) {
    for (i, (index, render)) in math_boxes.iter().enumerate() {
        builder.push_inline_box(InlineBox {
            id: MATH_ID_BASE + i as u64,
            kind: InlineBoxKind::InFlow,
            index: *index,
            width: render.width,
            height: render.ascent,
        });
    }
}

/// Extracted so the same styles can be applied in both the probe pass (pass 1)
/// and the final pass (pass 2) of the two-pass tab stop expansion.
pub(crate) fn push_para_styles(
    builder: &mut RangedBuilder<'_, LayoutColor>,
    para_props: &ResolvedParaProps,
    style_spans: &[StyleSpan],
) {
    builder.push_default(StyleProperty::Brush(LayoutColor::BLACK));
    builder.push_default(StyleProperty::FontSize(12.0));
    // Table cells break over-long words to the column width (CSS
    // `overflow-wrap: anywhere`); body paragraphs keep words intact.
    if para_props.break_long_words {
        builder.push_default(StyleProperty::OverflowWrap(OverflowWrap::Anywhere));
    }
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
        // Math placeholder spans have an empty range and no text — they are
        // typeset separately and reserved via an inline box, so they push no
        // text styles here.
        if span.math.is_some() {
            continue;
        }
        let r = span.range.clone();
        // For super/subscript (gap #3), reduce font size to 58 %. The shift is
        // applied in `para_emit` (`va_offset`) — TODO(super-sub): no native API.
        let effective_font_size = if span.vertical_align.is_some() {
            span.font_size * 0.58
        } else {
            span.font_size
        };
        builder.push(StyleProperty::FontSize(effective_font_size), r.clone());
        builder.push(StyleProperty::Brush(span.color), r.clone());
        // Push the effective numeric weight. `weight` already folds in `bold`
        // (700 when bold, else 400) plus any explicit `font_weight` style, so a
        // non-default weight is honoured even when the bold flag is unset.
        if span.weight != 400 {
            builder.push(
                StyleProperty::FontWeight(FontWeight::new(span.weight as f32)),
                r.clone(),
            );
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
        // Kerning (gap #23): the reference apps default pair kerning OFF
        // (see StyleSpan::kerning); harfrust defaults it ON — disable unless
        // the document explicitly enables it.
        if span.kerning != Some(true) {
            builder.push(
                StyleProperty::FontFeatures(FontFeatures::from("\"kern\" 0")),
                r.clone(),
            );
        }
        // Caps variant (gaps #15, #16): SmallCaps stored but not applied (no Parley API).
        // AllCaps: text was already uppercased during flatten_paragraph.
    }
}
