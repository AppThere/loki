// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 AppThere Loki contributors

//! Paragraph-level layout using Parley.
//!
//! [`layout_paragraph`] takes a flattened text string with ranged
//! [`StyleSpan`]s and paragraph properties, runs Parley shaping and
//! line-breaking, then converts the result into renderer-agnostic
//! [`PositionedItem`]s whose origins are relative to the paragraph's
//! own `(0, 0)` top-left corner.

use std::ops::Range;
use std::sync::Arc;

use parley::{AlignmentOptions, InlineBox, InlineBoxKind, PositionedLayoutItem};

use crate::font::FontResources;
use crate::geometry::LayoutRect;
use crate::items::{PositionedBorderRect, PositionedItem, PositionedRect};

#[path = "para_build.rs"]
mod build;
#[path = "para_layout_types.rs"]
mod layout_types;
#[path = "para_query.rs"]
mod query;
#[path = "para_tabs.rs"]
mod tabs;
#[path = "para_types.rs"]
mod types;
#[path = "para_underlays.rs"]
mod underlays;

pub use layout_types::{
    Affinity, CursorRect, HitTestResult, ParagraphLayout, ResolvedParaProps, WrapBand,
};
pub use types::{
    FontVariant, ResolvedLineHeight, ResolvedListMarker, ResolvedTabStop, StrikethroughStyle,
    StyleSpan, UnderlineStyle, VerticalAlign,
};

use build::push_math_inline_boxes;
pub(crate) use build::push_para_styles;

/// Strips characters Parley must not see (control chars and the BOM, keeping
/// `\t`/`\n`) and remaps the style spans onto the cleaned text.
///
/// Returns `(clean_text, clean_spans, orig_to_clean, clean_to_orig)` — the two
/// byte-index maps let editor hit-testing translate between the original and
/// cleaned coordinate spaces.
fn clean_text_and_spans(
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

/// Inline-box id base for math placeholders, kept clear of the tab-stop ids
/// (which count up from 0) so the two can coexist in one paragraph.
const MATH_ID_BASE: u64 = 1 << 40;

/// Probe-only inline-box id base for decimal-separator markers (one per tab),
/// used to measure where the first `.` after a tab sits for decimal alignment.
const DEC_ID_BASE: u64 = 1 << 20;

/// Probe-only inline-box id for the end-of-text sentinel, used to measure the
/// trailing edge of the content following the last tab.
const END_ID: u64 = 1 << 30;

/// Lay out a single paragraph using Parley.
///
/// `text_content` is the flattened text from all inline runs. `style_spans`
/// maps byte ranges to resolved character properties. `available_width` is
/// the maximum line width in points. `display_scale` is the HiDPI scale
/// factor (use `1.0` for layout-only / headless use).
///
/// When `preserve_for_editing` is `true`, the Parley `Layout` object is
/// retained in [`ParagraphLayout::parley_layout`] so that subsequent editing
/// sessions can call [`ParagraphLayout::hit_test_point`] and
/// [`ParagraphLayout::cursor_rect`]. In read-only rendering mode pass
/// `false` to avoid the memory cost on large documents.
///
/// The result is memoised in `resources.para_cache`: when the same inputs are
/// laid out again (e.g. every paragraph except the edited one, on a keystroke)
/// the cached layout is cloned instead of re-shaped. See
/// [`crate::para_cache`].
pub fn layout_paragraph(
    resources: &mut FontResources,
    text_content: &str,
    style_spans: &[StyleSpan],
    para_props: &ResolvedParaProps,
    available_width: f32,
    display_scale: f32,
    preserve_for_editing: bool,
) -> ParagraphLayout {
    layout_paragraph_spelled(
        resources,
        text_content,
        style_spans,
        para_props,
        available_width,
        display_scale,
        preserve_for_editing,
        None,
    )
}

/// [`layout_paragraph`] with an optional spell checker.
///
/// When `spell` is `Some`, misspelled words emit [`DecorationKind::Spelling`]
/// squiggles. The checker's `generation` folds into the cache key so cached
/// layouts are reused only while the dictionary/word-lists are unchanged.
// One arg over the limit: the optional spell checker on the shaping hot path.
#[allow(clippy::too_many_arguments)]
pub(crate) fn layout_paragraph_spelled(
    resources: &mut FontResources,
    text_content: &str,
    style_spans: &[StyleSpan],
    para_props: &ResolvedParaProps,
    available_width: f32,
    display_scale: f32,
    preserve_for_editing: bool,
    spell: Option<&crate::SpellState>,
) -> ParagraphLayout {
    let spell_generation = spell.map_or(0, |s| s.generation);
    let key = crate::para_cache::para_key(
        text_content,
        style_spans,
        para_props,
        available_width,
        display_scale,
        preserve_for_editing,
        spell_generation,
    );
    if let Some(hit) = resources.para_cache.get(key) {
        return hit;
    }
    let result = layout_paragraph_uncached(
        resources,
        text_content,
        style_spans,
        para_props,
        available_width,
        display_scale,
        preserve_for_editing,
        spell,
    );
    resources.para_cache.put(key, result.clone());
    result
}

/// Prepends the paragraph's border and background-fill rects to `items` (so
/// they render beneath the text). The box spans the full indented width and the
/// paragraph height. Background is inserted last so it sits behind the border.
fn prepend_para_box(
    items: &mut Vec<PositionedItem>,
    para_props: &ResolvedParaProps,
    width: f32,
    height: f32,
) {
    let bw = width + para_props.indent_start + para_props.indent_end;
    let has_border = para_props.border_top.is_some()
        || para_props.border_right.is_some()
        || para_props.border_bottom.is_some()
        || para_props.border_left.is_some();
    if has_border {
        items.insert(
            0,
            PositionedItem::BorderRect(PositionedBorderRect {
                rect: LayoutRect::new(0.0, 0.0, bw, height),
                top: para_props.border_top,
                right: para_props.border_right,
                bottom: para_props.border_bottom,
                left: para_props.border_left,
            }),
        );
    }
    if let Some(bg) = para_props.background_color {
        items.insert(
            0,
            PositionedItem::FilledRect(PositionedRect {
                rect: LayoutRect::new(0.0, 0.0, bw, height),
                color: bg,
            }),
        );
    }
}

/// Lays out a single paragraph using Parley, without consulting or populating
/// the shaping cache. [`layout_paragraph`] wraps this with memoisation.
#[allow(clippy::too_many_arguments)] // one arg over: the optional spell checker.
fn layout_paragraph_uncached(
    resources: &mut FontResources,
    text_content: &str,
    style_spans: &[StyleSpan],
    para_props: &ResolvedParaProps,
    available_width: f32,
    display_scale: f32,
    preserve_for_editing: bool,
    spell: Option<&crate::SpellState>,
) -> ParagraphLayout {
    let (mut clean_text, mut clean_spans, mut orig_to_clean, mut clean_to_orig) =
        clean_text_and_spans(text_content, style_spans);

    for span in &mut clean_spans {
        if let Some(ref name) = span.font_name {
            span.font_name = Some(resources.resolve_font_name(name));
        }
    }

    // Typeset each `Inline::Math` placeholder span (empty range, `math: Some`)
    // into its own box before the tab/final passes, so its intrinsic size can
    // reserve inline space for the equation.
    let mut math_boxes: Vec<(usize, crate::math::MathRender)> = Vec::new();
    for span in &clean_spans {
        if let Some(mathml) = &span.math {
            let render = crate::math::layout_math(
                resources,
                mathml,
                span.font_size,
                span.color,
                display_scale,
            );
            if render.width > 0.0 {
                math_boxes.push((span.range.start, render));
            }
        }
    }
    // A paragraph that contains only math has empty text; give Parley a single
    // space so it still produces a line to anchor the inline box(es).
    if clean_text.is_empty() && !math_boxes.is_empty() {
        clean_text = " ".to_string();
    }

    if clean_text.is_empty() {
        if !preserve_for_editing {
            return ParagraphLayout {
                height: 0.0,
                width: 0.0,
                items: vec![],
                first_baseline: 0.0,
                last_baseline: 0.0,
                line_boundaries: vec![],
                parley_layout: None,
                orig_to_clean,
                clean_to_orig,
                indent_start: para_props.indent_start,
                indent_hanging: para_props.indent_hanging,
                drop_lines: 0,
                drop_shift: 0.0,
            };
        }
        // Build a phantom single-space layout so cursor_rect can return a
        // properly-sized caret for empty paragraphs.  The space forces Parley
        // to produce one line with the paragraph's resolved font metrics.
        // height/line_boundaries are left at zero so empty paragraphs do not
        // affect vertical flow — they remain un-clickable but navigable.
        let mut builder =
            resources
                .layout_cx
                .ranged_builder(&mut resources.font_cx, " ", display_scale, true);
        push_para_styles(&mut builder, para_props, &[]);
        let mut phantom = builder.build(" ");
        phantom.break_all_lines(Some(available_width));
        let first_baseline = phantom
            .lines()
            .next()
            .map(|l| l.metrics().baseline)
            .unwrap_or(0.0);
        return ParagraphLayout {
            height: 0.0,
            width: 0.0,
            items: vec![],
            first_baseline,
            last_baseline: first_baseline,
            line_boundaries: vec![],
            parley_layout: Some(Arc::new(phantom)),
            orig_to_clean,
            clean_to_orig,
            indent_start: para_props.indent_start,
            indent_hanging: para_props.indent_hanging,
            drop_lines: 0,
            drop_shift: 0.0,
        };
    }

    // NOTE(indent-hanging-width): Parley 0.6 does not expose per-line width
    // control. The first line of a hanging-indent paragraph wraps at the same
    // `line_w` as subsequent lines, meaning it gets `indent_hanging` px less
    // space than it should. Fix requires Parley to expose per-line measure.
    // Tracked: fidelity audit gap #8 (partial).
    let line_w = (available_width - para_props.indent_start - para_props.indent_end).max(0.0);

    // ── Tab stop expansion (gap #7) ───────────────────────────────────────────
    // Parley 0.8 has no native tab stop API. Two-pass approach:
    //   Pass 1 (probe): zero-width InlineBoxes at each \t → measure x-positions.
    //   Pass 2 (final): InlineBoxes sized to advance to the next tab stop.
    let tab_char_positions: Vec<usize> = clean_text
        .char_indices()
        .filter(|(_, c)| *c == '\t')
        .map(|(i, _)| i)
        .collect();

    // Byte offset of the first decimal separator after each tab (before the
    // next tab / end), for Decimal-aligned stops.
    let decimal_positions: Vec<Option<usize>> = tab_char_positions
        .iter()
        .enumerate()
        .map(|(i, &t)| {
            let end = tab_char_positions
                .get(i + 1)
                .copied()
                .unwrap_or(clean_text.len());
            clean_text[t + 1..end].find('.').map(|rel| t + 1 + rel)
        })
        .collect();

    let tab_plans: Vec<tabs::TabPlan> = if tab_char_positions.is_empty() {
        vec![]
    } else {
        let n = tab_char_positions.len();
        let mut probe = resources.layout_cx.ranged_builder(
            &mut resources.font_cx,
            &clean_text,
            display_scale,
            true,
        );
        push_para_styles(&mut probe, para_props, &clean_spans);
        for (idx, &pos) in tab_char_positions.iter().enumerate() {
            probe.push_inline_box(InlineBox {
                id: idx as u64,
                kind: InlineBoxKind::InFlow,
                index: pos,
                width: 0.0,
                height: 0.0,
            });
            if let Some(dpos) = decimal_positions[idx] {
                probe.push_inline_box(InlineBox {
                    id: DEC_ID_BASE + idx as u64,
                    kind: InlineBoxKind::InFlow,
                    index: dpos,
                    width: 0.0,
                    height: 0.0,
                });
            }
        }
        probe.push_inline_box(InlineBox {
            id: END_ID,
            kind: InlineBoxKind::InFlow,
            index: clean_text.len(),
            width: 0.0,
            height: 0.0,
        });
        push_math_inline_boxes(&mut probe, &math_boxes);
        let mut probe_layout = probe.build(&clean_text);
        probe_layout.break_all_lines(Some(line_w));

        let mut x_tab = vec![0.0f32; n];
        let mut line_tab = vec![usize::MAX; n];
        let mut x_dec = vec![f32::NAN; n];
        let mut x_end = 0.0f32;
        let mut line_end = usize::MAX;
        for (li, line) in probe_layout.lines().enumerate() {
            for item in line.items() {
                if let PositionedLayoutItem::InlineBox(pib) = item {
                    let id = pib.id;
                    if (id as usize) < n {
                        x_tab[id as usize] = pib.x;
                        line_tab[id as usize] = li;
                    } else if (DEC_ID_BASE..END_ID).contains(&id) {
                        let i = (id - DEC_ID_BASE) as usize;
                        if i < n {
                            x_dec[i] = pib.x;
                        }
                    } else if id == END_ID {
                        x_end = pib.x;
                        line_end = li;
                    }
                }
            }
        }
        tabs::compute_tab_plans(
            &para_props.tab_stops,
            para_props.indent_hanging,
            para_props.default_tab_stop,
            &x_tab,
            &line_tab,
            &x_dec,
            x_end,
            line_end,
        )
    };

    // ── Drop-cap preparation ──────────────────────────────────────────────────
    // The dropped initial spans several lines: it is removed from the body flow
    // and rendered separately, the first `n_lines` body lines narrowed/shifted
    // to clear it, and its bytes trimmed from `clean_text` (the orig↔clean maps
    // are rebased below to keep editor hit-testing aligned). Read-only paint
    // uses the precise two-pass band split; the editor (`preserve_for_editing`)
    // lays the body out as one uniform-narrow layout it hit-tests against.
    let drop_state: Option<(
        loki_doc_model::style::props::drop_cap::DropCap,
        String,
        StyleSpan,
    )> = para_props
        .drop_cap
        .filter(|_| tab_char_positions.is_empty() && math_boxes.is_empty())
        .and_then(|dc| {
            let k = crate::para_drop_cap::cap_byte_len(&clean_text, dc.length);
            if k == 0 || k >= clean_text.len() {
                return None; // no initial, or no body text would remain
            }
            let base = clean_spans
                .iter()
                .find(|s| s.range.start == 0 && s.range.end > 0)
                .or_else(|| clean_spans.first())
                .cloned()?;
            let cap_text = clean_text[..k].to_string();
            let (body, body_spans) =
                crate::para_drop_cap::trim_leading(&clean_text, &clean_spans, k);
            clean_text = body;
            clean_spans = body_spans;
            Some((dc, cap_text, base))
        });

    // Rebase the orig↔clean maps past the trimmed cap so the body layout's
    // offsets (which start after the cap) map back to the right original bytes
    // for editor hit-testing. The cap bytes [0, k) collapse to body offset 0;
    // body byte j corresponds to clean byte j + k.
    let drop_cap_bytes = drop_state
        .as_ref()
        .map(|(_, cap, _)| cap.len())
        .unwrap_or(0);
    if drop_cap_bytes > 0 {
        for v in orig_to_clean.iter_mut() {
            *v = v.saturating_sub(drop_cap_bytes);
        }
        let drain_to = drop_cap_bytes.min(clean_to_orig.len());
        clean_to_orig.drain(0..drain_to);
    }

    // ── Main (final) layout pass ──────────────────────────────────────────────
    let mut builder = resources.layout_cx.ranged_builder(
        &mut resources.font_cx,
        &clean_text,
        display_scale,
        true,
    );
    push_para_styles(&mut builder, para_props, &clean_spans);
    for (idx, &pos) in tab_char_positions.iter().enumerate() {
        let width = tab_plans.get(idx).map(|p| p.width).unwrap_or(0.0);
        builder.push_inline_box(InlineBox {
            id: idx as u64,
            kind: InlineBoxKind::InFlow,
            index: pos,
            width,
            height: 0.0,
        });
    }
    push_math_inline_boxes(&mut builder, &math_boxes);

    let mut layout = builder.build(&clean_text);
    // Plan the drop cap (its enlarged glyph + band geometry) from the body's
    // first-line metrics. `drop_plan` keeps the line height for `cover_height`.
    let drop_plan = if let Some((dc, cap_text, base)) = &drop_state {
        layout.break_all_lines(Some(line_w)); // metrics only
        let (lh, asc, bl) = layout
            .lines()
            .next()
            .map(|l| {
                let m = l.metrics();
                (m.line_height, m.ascent, m.baseline)
            })
            .unwrap_or((0.0, 0.0, 0.0));
        crate::para_drop_cap::plan_drop_cap(
            resources,
            cap_text,
            base,
            dc,
            lh,
            bl,
            asc,
            display_scale,
        )
        .map(|p| (p, lh))
    } else {
        None
    };

    // Unified leading band: a drop cap (object on the left) or a float band set
    // by the flow engine. The band's first lines are narrowed; lines below it
    // reclaim full width (`para_band` lays the body out in two passes).
    let band: Option<crate::para_band::Band> = if let Some((p, lh)) = &drop_plan {
        Some(crate::para_band::Band {
            inset: p.body_inset,
            cover_height: p.n_lines as f32 * lh,
            // In-text drop shifts the text right; margin drop has inset 0.
            shift_text: p.body_inset > 0.0,
        })
    } else {
        para_props.wrap_band.map(|w| crate::para_band::Band {
            inset: w.inset,
            cover_height: w.cover_height,
            shift_text: w.shift_text,
        })
    };

    // Precise per-line band split runs on the read-only paint path for plain
    // text; the editor / tab / math paths fall back to a uniform narrow below.
    let can_split = !preserve_for_editing && tab_char_positions.is_empty() && math_boxes.is_empty();

    if let Some(band) = band.as_ref().filter(|_| can_split) {
        let body = crate::para_band::layout_band_body(
            resources,
            &clean_text,
            &clean_spans,
            para_props,
            line_w,
            display_scale,
            band,
        );
        let mut items = body.items;
        let mut content_bottom = body.height;
        if let Some((p, _)) = &drop_plan {
            // Emit the enlarged initial at the paragraph's left edge.
            for it in &p.items {
                let mut it = it.clone();
                it.translate(para_props.indent_start, 0.0);
                items.push(it);
            }
            content_bottom = content_bottom.max(p.bottom);
        }
        prepend_para_box(&mut items, para_props, body.width, body.height);
        return ParagraphLayout {
            height: content_bottom,
            width: body.width,
            items,
            first_baseline: body.first_baseline,
            last_baseline: body.last_baseline,
            line_boundaries: body.line_boundaries,
            parley_layout: None,
            orig_to_clean,
            clean_to_orig,
            indent_start: para_props.indent_start,
            indent_hanging: para_props.indent_hanging,
            drop_lines: 0,
            drop_shift: 0.0,
        };
    }

    // Fallback / normal path: break at the (possibly band-narrowed) width. A
    // band here is a drop cap in the editor, or a float that could not be split
    // (editor, tabs, or math). Every line wraps at the narrowed width (APPROX,
    // as documented for `para_band`); only the leading lines beside the object
    // are shifted right to clear it.
    let band_inset = band.as_ref().map(|b| b.inset).unwrap_or(0.0);
    let drop_shift = band
        .as_ref()
        .map(|b| if b.shift_text { b.inset } else { 0.0 })
        .unwrap_or(0.0);
    layout.break_all_lines(Some((line_w - band_inset).max(1.0)));
    layout.align(para_props.alignment, AlignmentOptions::default());

    // Leading lines whose top is within the band's vertical extent are shifted.
    let drop_lines = match &band {
        Some(b) => layout
            .lines()
            .take_while(|l| l.metrics().block_min_coord < b.cover_height)
            .count(),
        None => 0,
    };

    let total_height = layout.height();
    let total_width = layout.width();
    let first_baseline = layout
        .lines()
        .next()
        .map(|l| l.metrics().baseline)
        .unwrap_or(0.0);
    let last_baseline = layout
        .lines()
        .last()
        .map(|l| l.metrics().baseline)
        .unwrap_or(0.0);
    let line_boundaries: Vec<(f32, f32)> = layout
        .lines()
        .map(|l| (l.metrics().block_min_coord, l.metrics().block_max_coord))
        .collect();

    let mut items: Vec<PositionedItem> = Vec::new();
    let mut line_index: usize = 0;
    // Track the lowest point reached by any inline equation: a deep denominator
    // can hang below the line's descent; grow the paragraph height to cover it.
    let mut content_bottom = total_height;

    // OOXML lineRule="exact" (ODF fixed line height): the line box is a fixed
    // height and content taller than it is clipped — unlike "atLeast", which
    // grows. Each line's items are wrapped in a clip layer sized to the exact
    // line box so over-tall glyphs / inline objects are cut off as in Word.
    let exact_line_pts = match para_props.line_height {
        Some(ResolvedLineHeight::Exact(pts)) => Some(pts),
        _ => None,
    };

    // Highlight/background underlay (gap #10) and spelling squiggles: resolved
    // via Parley selection geometry and emitted behind the glyph runs.
    underlays::emit_highlight_underlays(
        &mut items,
        &layout,
        &clean_spans,
        para_props,
        drop_lines,
        drop_shift,
    );
    underlays::emit_spelling_squiggles(
        &mut items,
        &layout,
        &clean_text,
        &clean_spans,
        spell,
        para_props,
        drop_lines,
        drop_shift,
    );
    underlays::emit_para_mark_deletion(&mut items, &layout, para_props, drop_lines, drop_shift);

    for line in layout.lines() {
        // Index into `items` where this line's emitted items begin (used to wrap
        // them in a clip layer for exact line height).
        let line_item_start = items.len();
        // Hanging indent: the first line shifts left so the marker is visible to
        // the left of `indent_start`. Subsequent lines use the full `indent_start`.
        let mut indent_x = if line_index == 0 && para_props.indent_hanging > 0.0 {
            para_props.indent_start - para_props.indent_hanging
        } else {
            para_props.indent_start
        };
        // Leading lines beside a drop cap / float band are shifted right to
        // clear it; lines below it return to the paragraph's left edge.
        if line_index < drop_lines {
            indent_x += drop_shift;
        }
        let line_baseline = line.metrics().baseline;
        // Extra horizontal offset accumulated from horizontally-scaled (w:w)
        // runs earlier on this line, so later items shift right by the width the
        // scaling added instead of overlapping. Reset per line.
        let mut extra_x = 0.0f32;
        for item in line.items() {
            // Math inline box: emit the typeset equation's draw items, offset to
            // the box's resolved position on the line.
            if let PositionedLayoutItem::InlineBox(pib) = &item {
                if pib.id >= MATH_ID_BASE {
                    let mi = (pib.id - MATH_ID_BASE) as usize;
                    if let Some((_, render)) = math_boxes.get(mi) {
                        for prim in &render.items {
                            let mut prim = prim.clone();
                            prim.translate(pib.x + indent_x + extra_x, pib.y);
                            items.push(prim);
                        }
                        // The box top is at `pib.y` and its baseline at
                        // `pib.y + ascent`; the descent hangs below that.
                        content_bottom = content_bottom.max(pib.y + render.ascent + render.descent);
                    }
                } else if (pib.id as usize) < tab_char_positions.len() {
                    // Tab inline box: draw the stop's leader (if any) across the
                    // gap the box opened.
                    if let Some(plan) = tab_plans.get(pib.id as usize) {
                        tabs::emit_tab_leader(
                            &mut items,
                            plan.leader,
                            pib.x + indent_x + extra_x,
                            pib.x + indent_x + extra_x + pib.width,
                            line_baseline,
                        );
                    }
                }
                continue;
            }
            let PositionedLayoutItem::GlyphRun(glyph_run) = item else {
                continue;
            };
            let scale =
                span_scale_for_range(&clean_spans, glyph_run.run().text_range()).unwrap_or(1.0);
            // Reserve the extra width the run rendered (scaling, per-glyph or
            // uniform) so later runs on the line do not overlap.
            extra_x += crate::para_emit::emit_glyph_run(
                &glyph_run,
                indent_x + extra_x,
                &clean_spans,
                scale,
                resources,
                &mut items,
                // Highlights are emitted by the selection-geometry pass below.
                false,
            );
        }
        if let Some(pts) = exact_line_pts {
            // Clip this line's items to its fixed-height box. The clip is wide
            // horizontally (exact governs the vertical extent only; horizontal
            // overflow is handled by margins/wrapping, as in Word) and exactly
            // `pts` tall.
            //
            // Word anchors the exact line box at the BOTTOM of the text: the box
            // bottom sits at the baseline + descent and the top is `pts` above
            // it, so when the font is taller than `pts` the ascenders (and a
            // raised superscript) are clipped while descenders are preserved —
            // the well-known "tops cut off" behaviour of small exact spacing.
            // (A symmetric/centered box would instead clip descenders too, which
            // does not match Word.) Consecutive boxes still tile exactly because
            // Parley advances the baseline by `pts`.
            let lm = line.metrics();
            let top = lm.baseline + lm.descent - pts;
            let clipped: Vec<PositionedItem> = items.split_off(line_item_start);
            items.push(PositionedItem::ClippedGroup {
                clip_rect: LayoutRect::new(-line_w, top, line_w * 3.0, pts),
                items: clipped,
            });
        }
        line_index += 1;
    }

    // A drop cap reaches this fallback only in the editor (`preserve_for_editing`),
    // where the body is one hit-testable layout; emit its enlarged initial at the
    // paragraph's left edge, above the shifted body lines.
    if let Some((p, _)) = &drop_plan {
        for it in &p.items {
            let mut it = it.clone();
            it.translate(para_props.indent_start, 0.0);
            items.push(it);
        }
        content_bottom = content_bottom.max(p.bottom);
    }

    prepend_para_box(&mut items, para_props, total_width, total_height);

    let parley_layout = if preserve_for_editing {
        Some(Arc::new(layout))
    } else {
        None
    };

    ParagraphLayout {
        // `content_bottom` ≥ `total_height`; it is larger only when an inline
        // equation hangs below the last line (see above).
        height: content_bottom,
        width: total_width,
        items,
        first_baseline,
        last_baseline,
        line_boundaries,
        parley_layout,
        orig_to_clean,
        clean_to_orig,
        indent_start: para_props.indent_start,
        indent_hanging: para_props.indent_hanging,
        drop_lines,
        drop_shift,
    }
}

// List-marker synthesis lives in `crate::list_marker` (split from this
// file); re-exported here so `para::format_counter` callers and the
// `para_tests.rs` suite keep their existing paths.
pub(crate) use crate::list_marker::format_counter;
pub use crate::list_marker::format_list_marker;

// ── Private helpers for span → glyph-run lookups ──────────────────────────────

/// Returns the span whose byte range contains `offset`, or `None` if no span
/// covers it. Empty (zero-width) spans never match. Used by per-glyph emission
/// to resolve each glyph's scale / baseline shift.
pub(crate) fn span_at_offset(spans: &[StyleSpan], offset: usize) -> Option<&StyleSpan> {
    spans
        .iter()
        .find(|s| s.range.start <= offset && offset < s.range.end)
}

/// Returns the horizontal text scale for the first span fully containing
/// `text_range`, or `None` when the run is unscaled (100 %).
pub(crate) fn span_scale_for_range(spans: &[StyleSpan], text_range: Range<usize>) -> Option<f32> {
    spans
        .iter()
        .find(|s| s.range.start <= text_range.start && s.range.end >= text_range.end)
        .and_then(|s| s.scale)
}

#[cfg(test)]
#[path = "para_tests.rs"]
mod tests;
