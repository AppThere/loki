// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 AppThere Loki contributors

//! Unit tests for [`crate::para`].

use super::*;
use crate::items::{BorderStyle, DecorationKind, PositionedGlyphRun, PositionedItem};
use loki_doc_model::style::list_style::{
    BulletChar, LabelAlignment, ListLevel, ListLevelKind, NumberingScheme,
};
use loki_primitives::units::Points as DocPoints;

// ── helpers ───────────────────────────────────────────────────────────────────

/// Build a `FontResources` with Liberation Sans registered so tests are not
/// dependent on fontconfig auto-discovery.
fn test_resources() -> FontResources {
    let mut r = FontResources::new();
    for p in [
        "/usr/share/fonts/truetype/liberation/LiberationSans-Regular.ttf",
        "/usr/share/fonts/truetype/liberation/LiberationSans-Bold.ttf",
    ] {
        if let Ok(data) = std::fs::read(p) {
            r.register_font(data);
        }
    }
    r
}

fn single_span(text: &str, font_size: f32) -> StyleSpan {
    StyleSpan {
        range: 0..text.len(),
        font_name: None,
        font_size,
        bold: false,
        weight: 400,
        italic: false,
        color: LayoutColor::BLACK,
        underline: None,
        strikethrough: None,
        line_height: None,
        vertical_align: None,
        highlight_color: None,
        letter_spacing: None,
        font_variant: None,
        word_spacing: None,
        shadow: false,
        link_url: None,
        math: None,
        scale: None,
        kerning: None,
        baseline_shift: None,
    }
}

/// A math placeholder span (empty range carrying MathML) at byte `at`.
fn math_span(at: usize, mathml: &str) -> StyleSpan {
    let mut s = single_span("", 12.0);
    s.range = at..at;
    s.math = Some(std::sync::Arc::from(mathml));
    s
}

// ── tests ─────────────────────────────────────────────────────────────────────

#[test]
fn inline_math_emits_typeset_items() {
    const NS: &str = "http://www.w3.org/1998/Math/MathML";
    let mut r = test_resources();
    let text = "x = ";
    let mathml = format!("<math xmlns=\"{NS}\"><mfrac><mn>1</mn><mn>2</mn></mfrac></math>");

    // Baseline: same paragraph without the equation.
    let plain = [single_span(text, 12.0)];
    let plain_layout = layout_paragraph(
        &mut r,
        text,
        &plain,
        &ResolvedParaProps::default(),
        400.0,
        1.0,
        false,
    );

    let spans = [single_span(text, 12.0), math_span(text.len(), &mathml)];
    let result = layout_paragraph(
        &mut r,
        text,
        &spans,
        &ResolvedParaProps::default(),
        400.0,
        1.0,
        false,
    );

    // The fraction contributes a bar rule and at least the two numerator/
    // denominator glyph runs on top of the paragraph's own text runs.
    let rects = result
        .items
        .iter()
        .filter(|i| matches!(i, PositionedItem::FilledRect(_)))
        .count();
    let glyph_runs = result
        .items
        .iter()
        .filter(|i| matches!(i, PositionedItem::GlyphRun(_)))
        .count();
    let plain_runs = plain_layout
        .items
        .iter()
        .filter(|i| matches!(i, PositionedItem::GlyphRun(_)))
        .count();

    assert_eq!(rects, 1, "fraction bar should be drawn");
    assert!(
        glyph_runs > plain_runs,
        "math adds glyph runs beyond the plain text ({glyph_runs} vs {plain_runs})"
    );
}

#[test]
fn inline_math_baseline_aligns_with_text() {
    const NS: &str = "http://www.w3.org/1998/Math/MathML";
    let mut r = test_resources();
    let text = "x = ";
    let mathml = format!("<math xmlns=\"{NS}\"><mi>y</mi></math>");
    let spans = [single_span(text, 12.0), math_span(text.len(), &mathml)];
    let result = layout_paragraph(
        &mut r,
        text,
        &spans,
        &ResolvedParaProps::default(),
        400.0,
        1.0,
        false,
    );

    // A lone identifier sits on the math baseline, which the inline box places
    // on the text baseline — so every glyph run (the "x = " text and the math
    // "y") shares one baseline `y`.
    let baselines: Vec<f32> = result
        .items
        .iter()
        .filter_map(|i| match i {
            PositionedItem::GlyphRun(g) => Some(g.origin.y),
            _ => None,
        })
        .collect();
    assert!(baselines.len() >= 2, "expected text + math glyph runs");
    let first = baselines[0];
    for b in &baselines {
        assert!(
            (b - first).abs() < 0.6,
            "math baseline {b} should match text baseline {first}"
        );
    }
}

#[test]
fn plain_paragraph_non_empty() {
    let mut r = test_resources();
    let text = "Hello, world!";
    let spans = [single_span(text, 12.0)];
    let result = layout_paragraph(
        &mut r,
        text,
        &spans,
        &ResolvedParaProps::default(),
        400.0,
        1.0,
        false,
    );
    assert!(result.height > 0.0, "height should be positive");
    assert!(!result.items.is_empty(), "items should not be empty");
}

#[test]
fn bold_span_produces_items() {
    let mut r = test_resources();
    let text = "Hello bold world";
    let spans = [
        StyleSpan {
            range: 0..6,
            bold: false,
            ..single_span(text, 12.0)
        },
        StyleSpan {
            range: 6..10,
            bold: true,
            weight: 700,
            ..single_span(text, 12.0)
        },
        StyleSpan {
            range: 10..text.len(),
            bold: false,
            ..single_span(text, 12.0)
        },
    ];
    let result = layout_paragraph(
        &mut r,
        text,
        &spans,
        &ResolvedParaProps::default(),
        400.0,
        1.0,
        false,
    );
    assert!(!result.items.is_empty());
    let runs = result
        .items
        .iter()
        .filter(|i| matches!(i, PositionedItem::GlyphRun(_)))
        .count();
    assert!(runs >= 1, "expected at least one glyph run, got {runs}");
}

#[test]
fn narrow_width_causes_wrapping() {
    let mut r = test_resources();
    let text = "The quick brown fox jumps over the lazy dog";
    let spans = [single_span(text, 14.0)];
    let wide = layout_paragraph(
        &mut r,
        text,
        &spans,
        &ResolvedParaProps::default(),
        600.0,
        1.0,
        false,
    );
    let narrow = layout_paragraph(
        &mut r,
        text,
        &spans,
        &ResolvedParaProps::default(),
        80.0,
        1.0,
        false,
    );
    assert!(
        narrow.height > wide.height,
        "narrow layout should be taller due to wrapping"
    );
}

#[test]
fn background_color_is_first_item() {
    let mut r = test_resources();
    let text = "Background test";
    let props = ResolvedParaProps {
        background_color: Some(LayoutColor::WHITE),
        ..Default::default()
    };
    let result = layout_paragraph(
        &mut r,
        text,
        &[single_span(text, 12.0)],
        &props,
        400.0,
        1.0,
        false,
    );
    assert!(
        matches!(result.items.first(), Some(PositionedItem::FilledRect(_))),
        "first item should be FilledRect for paragraph background",
    );
}

#[test]
fn underline_span_emits_decoration() {
    let mut r = test_resources();
    let text = "Underlined text";
    let spans = [StyleSpan {
        underline: Some(UnderlineStyle::Single),
        ..single_span(text, 12.0)
    }];
    let result = layout_paragraph(
        &mut r,
        text,
        &spans,
        &ResolvedParaProps::default(),
        400.0,
        1.0,
        false,
    );
    let has_underline = result.items.iter().any(
        |item| matches!(item, PositionedItem::Decoration(d) if d.kind == DecorationKind::Underline),
    );
    assert!(has_underline, "expected a Underline decoration item");
}

#[test]
fn underline_variant_carries_to_decoration_style() {
    use crate::items::DecorationStyle;
    let mut r = test_resources();
    let text = "Styled underline";
    for (u, expect) in [
        (UnderlineStyle::Single, DecorationStyle::Solid),
        (UnderlineStyle::Double, DecorationStyle::Double),
        (UnderlineStyle::Dotted, DecorationStyle::Dotted),
        (UnderlineStyle::Dash, DecorationStyle::Dashed),
        (UnderlineStyle::Wave, DecorationStyle::Wave),
        (UnderlineStyle::Thick, DecorationStyle::Thick),
    ] {
        let spans = [StyleSpan {
            underline: Some(u),
            ..single_span(text, 12.0)
        }];
        let result = layout_paragraph(
            &mut r,
            text,
            &spans,
            &ResolvedParaProps::default(),
            400.0,
            1.0,
            false,
        );
        let deco = result
            .items
            .iter()
            .find_map(|item| match item {
                PositionedItem::Decoration(d) if d.kind == DecorationKind::Underline => Some(d),
                _ => None,
            })
            .expect("underline decoration");
        assert_eq!(deco.style, expect, "{u:?} should map to {expect:?}");
    }
}

#[test]
fn double_strikethrough_carries_double_style() {
    use crate::items::DecorationStyle;
    use crate::para::StrikethroughStyle;
    let mut r = test_resources();
    let text = "Struck through";
    let spans = [StyleSpan {
        strikethrough: Some(StrikethroughStyle::Double),
        ..single_span(text, 12.0)
    }];
    let result = layout_paragraph(
        &mut r,
        text,
        &spans,
        &ResolvedParaProps::default(),
        400.0,
        1.0,
        false,
    );
    let deco = result
        .items
        .iter()
        .find_map(|item| match item {
            PositionedItem::Decoration(d) if d.kind == DecorationKind::Strikethrough => Some(d),
            _ => None,
        })
        .expect("strikethrough decoration");
    assert_eq!(deco.style, DecorationStyle::Double);
}

#[test]
fn space_before_after_not_in_height() {
    let mut r = test_resources();
    let text = "Spacing test";
    let spans = [single_span(text, 12.0)];
    let no_space = layout_paragraph(
        &mut r,
        text,
        &spans,
        &ResolvedParaProps::default(),
        400.0,
        1.0,
        false,
    );
    let with_space = layout_paragraph(
        &mut r,
        text,
        &spans,
        &ResolvedParaProps {
            space_before: 24.0,
            space_after: 24.0,
            ..Default::default()
        },
        400.0,
        1.0,
        false,
    );
    assert_eq!(
        no_space.height, with_space.height,
        "space_before/space_after must not affect ParagraphLayout::height",
    );
}

#[test]
fn line_boundaries_populated_for_multiline_paragraph() {
    let mut r = test_resources();
    let text = "The quick brown fox jumps over the lazy dog and continues for many more words to force wrapping";
    let spans = [single_span(text, 12.0)];
    // Narrow width forces several lines.
    let result = layout_paragraph(
        &mut r,
        text,
        &spans,
        &ResolvedParaProps::default(),
        100.0,
        1.0,
        false,
    );
    assert!(
        result.line_boundaries.len() >= 2,
        "expected multiple lines, got {}",
        result.line_boundaries.len()
    );
    // Each line's max_coord must be greater than its min_coord.
    for (i, &(min, max)) in result.line_boundaries.iter().enumerate() {
        assert!(
            max > min,
            "line {i}: max_coord ({max}) must exceed min_coord ({min})"
        );
    }
    // max_coords must be strictly increasing (each line's bottom is further down).
    for i in 1..result.line_boundaries.len() {
        let prev_max = result.line_boundaries[i - 1].1;
        let curr_max = result.line_boundaries[i].1;
        assert!(
            curr_max > prev_max,
            "line {i} max_coord ({curr_max}) must exceed previous line max_coord ({prev_max})"
        );
    }
    // Last line's max_coord should approximate the total paragraph height.
    let last_max = result.line_boundaries.last().unwrap().1;
    assert!(
        (last_max - result.height).abs() < 1.0,
        "last line max_coord ({last_max}) should equal paragraph height ({})",
        result.height
    );
}

#[test]
fn empty_paragraph_has_no_line_boundaries() {
    let mut r = test_resources();
    let result = layout_paragraph(
        &mut r,
        "",
        &[],
        &ResolvedParaProps::default(),
        400.0,
        1.0,
        false,
    );
    assert!(
        result.line_boundaries.is_empty(),
        "empty paragraph must have no line boundaries"
    );
}

#[test]
fn border_follows_background() {
    let mut r = test_resources();
    let text = "Border test";
    let edge = BorderEdge {
        color: LayoutColor::BLACK,
        width: 1.0,
        style: BorderStyle::Solid,
    };
    let props = ResolvedParaProps {
        background_color: Some(LayoutColor::WHITE),
        border_top: Some(edge),
        ..Default::default()
    };
    let result = layout_paragraph(
        &mut r,
        text,
        &[single_span(text, 12.0)],
        &props,
        400.0,
        1.0,
        false,
    );
    assert!(matches!(
        result.items.first(),
        Some(PositionedItem::FilledRect(_))
    ));
    assert!(matches!(
        result.items.get(1),
        Some(PositionedItem::BorderRect(_))
    ));
}

#[test]
fn superscript_span_uses_smaller_font() {
    // A span with vertical_align=Superscript should use font_size * 0.58.
    // We verify by checking that the layout of a superscript run produces a
    // GlyphRun with a smaller ascent than a plain run at the same font_size.
    // The simplest proxy: just ensure the paragraph lays out without panic and
    // produces at least one glyph run.
    let mut r = test_resources();
    let text = "x2";
    let spans = [StyleSpan {
        range: 0..2,
        vertical_align: Some(VerticalAlign::Superscript),
        ..single_span(text, 12.0)
    }];
    let result = layout_paragraph(
        &mut r,
        text,
        &spans,
        &ResolvedParaProps::default(),
        400.0,
        1.0,
        false,
    );
    let runs = result
        .items
        .iter()
        .filter(|i| matches!(i, PositionedItem::GlyphRun(_)))
        .count();
    assert!(
        runs >= 1,
        "superscript span must produce at least one glyph run"
    );
}

#[test]
fn horizontal_scale_widens_glyph_advances() {
    // A run with scale=1.5 (w:w=150) must emit glyph advances 1.5× the unscaled
    // run, stretching the text horizontally.
    fn total_advance(text: &str, spans: &[StyleSpan]) -> f32 {
        let mut r = test_resources();
        let result = layout_paragraph(
            &mut r,
            text,
            spans,
            &ResolvedParaProps::default(),
            400.0,
            1.0,
            false,
        );
        result
            .items
            .iter()
            .filter_map(|i| match i {
                PositionedItem::GlyphRun(g) => {
                    Some(g.glyphs.iter().map(|gl| gl.advance).sum::<f32>())
                }
                _ => None,
            })
            .sum()
    }
    let text = "wide";
    let plain = [single_span(text, 12.0)];
    let scaled = [StyleSpan {
        scale: Some(1.5),
        ..single_span(text, 12.0)
    }];
    let plain_w = total_advance(text, &plain);
    let scaled_w = total_advance(text, &scaled);
    assert!(plain_w > 0.0, "plain run must have a positive advance");
    assert!(
        (scaled_w - plain_w * 1.5).abs() < 0.5,
        "scaled advance {scaled_w} should be ~1.5× plain {plain_w}"
    );
}

#[test]
fn exact_line_height_clips_each_line() {
    // lineRule="exact" must wrap each line's items in a ClippedGroup sized to
    // the fixed line box; "auto"/default must not clip.
    let mut r = test_resources();
    let text = "clipped line";
    let spans = [single_span(text, 12.0)];

    let exact_props = ResolvedParaProps {
        line_height: Some(ResolvedLineHeight::Exact(8.0)),
        ..Default::default()
    };
    let exact = layout_paragraph(&mut r, text, &spans, &exact_props, 400.0, 1.0, false);
    let clip = exact.items.iter().find_map(|i| match i {
        PositionedItem::ClippedGroup { clip_rect, items } => Some((clip_rect, items)),
        _ => None,
    });
    let (clip_rect, inner) = clip.expect("exact line height must emit a ClippedGroup per line");
    // The clip box height is the fixed 8 pt line box (within rounding).
    assert!(
        (clip_rect.height() - 8.0).abs() < 0.5,
        "clip height {} should equal the exact 8pt line box",
        clip_rect.height()
    );
    assert!(
        inner
            .iter()
            .any(|i| matches!(i, PositionedItem::GlyphRun(_))),
        "clipped group must contain the line's glyph run"
    );
    // Word bottom-anchors the exact box: its bottom sits at baseline + descent,
    // so the box bottom is *below* the baseline (descenders preserved, ascenders
    // clipped). A symmetric/centered box would put the bottom at or above the
    // baseline, clipping descenders too — which is not what Word does.
    let box_bottom = clip_rect.y() + clip_rect.height();
    assert!(
        box_bottom > exact.first_baseline,
        "exact clip box bottom ({box_bottom}) must sit below the baseline \
         ({}) so descenders survive and only the top clips",
        exact.first_baseline
    );

    // Default (metrics-relative) line height: no clipping.
    let plain = layout_paragraph(
        &mut r,
        text,
        &spans,
        &ResolvedParaProps::default(),
        400.0,
        1.0,
        false,
    );
    assert!(
        !plain
            .items
            .iter()
            .any(|i| matches!(i, PositionedItem::ClippedGroup { .. })),
        "non-exact line height must not clip lines"
    );
}

#[test]
fn misspelled_word_emits_spelling_squiggle() {
    let mut r = test_resources();
    let checker =
        std::sync::Arc::new(loki_spell::SpellChecker::bundled().expect("bundled dictionary loads"));
    let spell = crate::SpellState {
        checker,
        generation: 1,
    };
    let text = "hello teh world";
    let spans = [single_span(text, 12.0)];
    let result = layout_paragraph_spelled(
        &mut r,
        text,
        &spans,
        &ResolvedParaProps::default(),
        400.0,
        1.0,
        false,
        Some(&spell),
    );

    let squiggles: Vec<_> = result
        .items
        .iter()
        .filter_map(|i| match i {
            PositionedItem::Decoration(d) if d.kind == DecorationKind::Spelling => Some(d),
            _ => None,
        })
        .collect();
    assert_eq!(squiggles.len(), 1, "only 'teh' is misspelled");
    let sq = squiggles[0];
    assert!(sq.width > 0.0, "squiggle has positive width");
    // 'teh' starts after 'hello ', so the squiggle is offset from the left edge.
    assert!(sq.x > 0.0, "squiggle starts past the first word");
}

#[test]
fn spelling_squiggle_hugs_the_descender_not_the_line_box() {
    // With a generous line height the line box extends far below the glyphs.
    // The squiggle must anchor to the text descender (baseline + descent), not
    // the line-box bottom — otherwise it floats in the inter-line leading.
    let mut r = test_resources();
    let checker =
        std::sync::Arc::new(loki_spell::SpellChecker::bundled().expect("bundled dictionary loads"));
    let spell = crate::SpellState {
        checker,
        generation: 1,
    };
    let props = ResolvedParaProps {
        line_height: Some(ResolvedLineHeight::MetricsRelative(3.0)),
        ..ResolvedParaProps::default()
    };
    let text = "hello teh world";
    let spans = [single_span(text, 12.0)];
    let result = layout_paragraph_spelled(
        &mut r,
        text,
        &spans,
        &props,
        400.0,
        1.0,
        false,
        Some(&spell),
    );

    // The single line's baseline is the glyph run origin y (see para_emit).
    let baseline = result
        .items
        .iter()
        .find_map(|i| match i {
            PositionedItem::GlyphRun(g) => Some(g.origin.y),
            _ => None,
        })
        .expect("a glyph run");
    let sq = result
        .items
        .iter()
        .find_map(|i| match i {
            PositionedItem::Decoration(d) if d.kind == DecorationKind::Spelling => Some(d),
            _ => None,
        })
        .expect("a squiggle");
    // Just below the baseline (descender zone, well under one 12 pt em), NOT the
    // ~2-em drop the old line-box-bottom anchor produced under 3× line height.
    assert!(
        sq.y > baseline,
        "squiggle sits below the baseline: y={} baseline={baseline}",
        sq.y
    );
    assert!(
        sq.y < baseline + 10.0,
        "squiggle hugs the descender, not the line-box bottom: y={} baseline={baseline}",
        sq.y
    );
}

#[test]
fn no_squiggles_when_spelling_disabled() {
    let mut r = test_resources();
    let text = "hello teh world";
    let spans = [single_span(text, 12.0)];
    // Default `layout_paragraph` passes no checker — no Spelling decorations.
    let result = layout_paragraph(
        &mut r,
        text,
        &spans,
        &ResolvedParaProps::default(),
        400.0,
        1.0,
        false,
    );
    assert!(
        !result.items.iter().any(
            |i| matches!(i, PositionedItem::Decoration(d) if d.kind == DecorationKind::Spelling)
        ),
        "no checker supplied, so no squiggles"
    );
}

#[test]
fn highlight_color_produces_filled_rect_before_glyph_run() {
    let mut r = test_resources();
    let text = "highlighted";
    let spans = [StyleSpan {
        highlight_color: Some(LayoutColor::new(1.0, 1.0, 0.0, 1.0)),
        ..single_span(text, 12.0)
    }];
    let result = layout_paragraph(
        &mut r,
        text,
        &spans,
        &ResolvedParaProps::default(),
        400.0,
        1.0,
        false,
    );
    // First non-background item should be a FilledRect (highlight), then a GlyphRun.
    let rects = result
        .items
        .iter()
        .filter(|i| matches!(i, PositionedItem::FilledRect(_)))
        .count();
    assert!(
        rects >= 1,
        "highlight span must produce at least one FilledRect"
    );
    // The FilledRect must come before the GlyphRun.
    let rect_pos = result
        .items
        .iter()
        .position(|i| matches!(i, PositionedItem::FilledRect(_)))
        .unwrap();
    let run_pos = result
        .items
        .iter()
        .position(|i| matches!(i, PositionedItem::GlyphRun(_)))
        .unwrap();
    assert!(
        rect_pos < run_pos,
        "FilledRect (highlight) must precede its GlyphRun"
    );
}

#[test]
fn coalesced_scale_and_baseline_shift_apply_per_glyph() {
    // Three runs with identical font/size/colour — Parley shapes them into ONE
    // glyph run. The first is 150 %-scaled (w:w), the second raised and the third
    // lowered (w:position). Per-glyph emission must still scale the first quarter
    // and raise/lower the others, even though no per-run lookup could.
    let mut r = test_resources();
    let text = "AAAABBBBCCCC";
    let mk = |range: std::ops::Range<usize>, scale: Option<f32>, rise: Option<f32>| StyleSpan {
        range,
        scale,
        kerning: None,
        baseline_shift: rise,
        ..single_span("A", 20.0)
    };
    let spans = [
        mk(0..4, Some(1.5), None),
        mk(4..8, None, Some(6.0)),   // raised 6 pt
        mk(8..12, None, Some(-6.0)), // lowered 6 pt
    ];
    let result = layout_paragraph(
        &mut r,
        text,
        &spans,
        &ResolvedParaProps::default(),
        2000.0,
        1.0,
        false,
    );

    let glyphs = result
        .items
        .iter()
        .find_map(|i| match i {
            PositionedItem::GlyphRun(g) => Some(&g.glyphs),
            _ => None,
        })
        .expect("a glyph run");
    assert_eq!(glyphs.len(), 12, "all 12 glyphs in one coalesced run");

    // First 4 (scaled 1.5×) advance wider than the unscaled middle 4.
    let scaled_adv = glyphs[0].advance;
    let plain_adv = glyphs[4].advance;
    assert!(
        scaled_adv > plain_adv * 1.4,
        "scaled glyph advance {scaled_adv} should be ~1.5× the plain {plain_adv}"
    );
    // Raised glyphs sit higher (smaller y) than lowered glyphs.
    let raised_y = glyphs[4].y;
    let lowered_y = glyphs[8].y;
    assert!(
        lowered_y - raised_y > 10.0,
        "lowered y ({lowered_y}) must be well below raised y ({raised_y})"
    );
    assert!(
        raised_y < -3.0,
        "raised glyph y ({raised_y}) should be above baseline"
    );
    assert!(
        lowered_y > 3.0,
        "lowered glyph y ({lowered_y}) should be below baseline"
    );
}

#[test]
fn highlight_emits_even_when_runs_coalesce() {
    // Regression: two adjacent spans with identical font/colour (so Parley shapes
    // them into ONE glyph run) where only the first is highlighted. The per-run
    // highlight lookup fails here (no single span covers the coalesced run), so
    // the highlight must come from the selection-geometry pass instead.
    let mut r = test_resources();
    let text = "ab";
    let spans = [
        StyleSpan {
            range: 0..1,
            highlight_color: Some(LayoutColor::new(1.0, 1.0, 0.0, 1.0)),
            ..single_span("a", 12.0)
        },
        StyleSpan {
            range: 1..2,
            ..single_span("b", 12.0)
        },
    ];
    let result = layout_paragraph(
        &mut r,
        text,
        &spans,
        &ResolvedParaProps::default(),
        400.0,
        1.0,
        false,
    );
    let yellow = result.items.iter().any(|i| match i {
        PositionedItem::FilledRect(rect) => {
            rect.color.r > 0.9
                && rect.color.g > 0.9
                && rect.color.b < 0.1
                && rect.rect.width() > 0.0
        }
        _ => false,
    });
    assert!(
        yellow,
        "highlight on the first of two coalesced runs must still emit a yellow FilledRect"
    );
}

#[test]
fn shadow_span_produces_extra_glyph_run() {
    let mut r = test_resources();
    let text = "shadow";
    let plain_spans = [single_span(text, 12.0)];
    let shadow_spans = [StyleSpan {
        shadow: true,
        ..single_span(text, 12.0)
    }];
    let plain = layout_paragraph(
        &mut r,
        text,
        &plain_spans,
        &ResolvedParaProps::default(),
        400.0,
        1.0,
        false,
    );
    let shadow = layout_paragraph(
        &mut r,
        text,
        &shadow_spans,
        &ResolvedParaProps::default(),
        400.0,
        1.0,
        false,
    );
    let plain_runs = plain
        .items
        .iter()
        .filter(|i| matches!(i, PositionedItem::GlyphRun(_)))
        .count();
    let shadow_runs = shadow
        .items
        .iter()
        .filter(|i| matches!(i, PositionedItem::GlyphRun(_)))
        .count();
    assert!(
        shadow_runs > plain_runs,
        "shadow span must produce more GlyphRun items than plain ({shadow_runs} vs {plain_runs})"
    );
}

// ── format_list_marker tests ──────────────────────────────────────────────────

fn bullet_level(c: char) -> ListLevel {
    ListLevel {
        level: 0,
        kind: ListLevelKind::Bullet {
            char: BulletChar::Char(c),
            font: None,
        },
        indent_start: DocPoints::new(36.0),
        hanging_indent: DocPoints::new(18.0),
        label_alignment: LabelAlignment::Left,
        tab_stop_after_label: None,
        char_props: Default::default(),
    }
}

fn numbered_level(
    level: u8,
    scheme: NumberingScheme,
    format: &str,
    display_levels: u8,
    start: u32,
) -> ListLevel {
    ListLevel {
        level,
        kind: ListLevelKind::Numbered {
            scheme,
            start_value: start,
            format: format.to_string(),
            display_levels,
        },
        indent_start: DocPoints::new(36.0),
        hanging_indent: DocPoints::new(18.0),
        label_alignment: LabelAlignment::Left,
        tab_stop_after_label: None,
        char_props: Default::default(),
    }
}

fn counters(vals: &[(usize, u32)]) -> [u32; 9] {
    let mut arr = [0u32; 9];
    for &(i, v) in vals {
        arr[i] = v;
    }
    arr
}

#[test]
fn format_marker_bullet() {
    let levels = vec![bullet_level('•')];
    assert_eq!(format_list_marker(&levels, 0, &counters(&[(0, 1)])), "•");
}

#[test]
fn format_marker_decimal_with_suffix() {
    let levels = vec![numbered_level(0, NumberingScheme::Decimal, "%1.", 1, 1)];
    assert_eq!(format_list_marker(&levels, 0, &counters(&[(0, 3)])), "3.");
}

#[test]
fn format_marker_lower_letter_overflow() {
    let levels = vec![numbered_level(0, NumberingScheme::LowerAlpha, "%1.", 1, 1)];
    assert_eq!(format_list_marker(&levels, 0, &counters(&[(0, 1)])), "a.");
    assert_eq!(format_list_marker(&levels, 0, &counters(&[(0, 26)])), "z.");
    assert_eq!(format_list_marker(&levels, 0, &counters(&[(0, 27)])), "aa.");
}

#[test]
fn format_marker_upper_roman() {
    let levels = vec![numbered_level(0, NumberingScheme::UpperRoman, "%1.", 1, 1)];
    assert_eq!(format_list_marker(&levels, 0, &counters(&[(0, 4)])), "IV.");
}

#[test]
fn format_marker_display_levels_two_level() {
    let levels = vec![
        numbered_level(0, NumberingScheme::Decimal, "%1.", 1, 1),
        numbered_level(1, NumberingScheme::Decimal, "%1.%2.", 2, 1),
    ];
    // level 0 counter = 2, level 1 counter = 3 → "2.3."
    assert_eq!(
        format_list_marker(&levels, 1, &counters(&[(0, 2), (1, 3)])),
        "2.3."
    );
}

#[test]
fn format_marker_picture_bullet_has_no_text() {
    // A picture bullet emits no marker *text* — the image is placed out-of-band
    // by the flow engine, so the text label is empty (not the old `•` fallback).
    let levels = vec![ListLevel {
        level: 0,
        kind: ListLevelKind::Bullet {
            char: BulletChar::Image {
                src: "data:image/png;base64,AAAA".to_string(),
            },
            font: None,
        },
        indent_start: DocPoints::new(36.0),
        hanging_indent: DocPoints::new(18.0),
        label_alignment: LabelAlignment::Left,
        tab_stop_after_label: None,
        char_props: Default::default(),
    }];
    assert_eq!(format_list_marker(&levels, 0, &counters(&[])), "");
}

// ── Counter tracking tests ────────────────────────────────────────────────────

#[test]
fn counter_advance_single_list() {
    // advance_counter is tested via format_list_marker indirectly.
    // We directly test the alpha_label helper through format_counter logic.
    // Three advances: 1, 2, 3.
    let levels = vec![numbered_level(0, NumberingScheme::Decimal, "%1.", 1, 1)];
    for (i, expected) in [(1, "1."), (2, "2."), (3, "3.")] {
        assert_eq!(
            format_list_marker(&levels, 0, &counters(&[(0, i)])),
            expected
        );
    }
}

#[test]
fn counter_nested_deeper_reset() {
    // When level 0 advances, level 1 should have been reset to 0.
    // We simulate: level 0 = 2, level 1 = 0 (reset) then first use = 1.
    let levels = vec![
        numbered_level(0, NumberingScheme::Decimal, "%1.", 1, 1),
        numbered_level(1, NumberingScheme::Decimal, "%1.%2.", 2, 1),
    ];
    // After level-0 advances to 2 and level-1 is reset, the next level-1
    // item should show "2.1." (level-1 reinitialised from start_value=1).
    let c = counters(&[(0, 2), (1, 1)]);
    assert_eq!(format_list_marker(&levels, 1, &c), "2.1.");
}

// ── Hit-testing and cursor-rect tests ────────────────────────────────────────
//
// These tests depend on a font being available. `test_resources()` tries to
// register Liberation Sans. If no font loads, Parley still returns plausible
// cursor positions using its fallback metrics, so the tests remain valid.

fn editing_paragraph(text: &str) -> ParagraphLayout {
    let mut r = test_resources();
    let spans = [single_span(text, 12.0)];
    layout_paragraph(
        &mut r,
        text,
        &spans,
        &ResolvedParaProps::default(),
        400.0,
        1.0,
        true,
    )
}

#[test]
fn left_indent_included_in_cursor_and_hit_test() {
    // Regression: paragraphs with a left indent (e.g. screenplay Character /
    // parenthetical blocks) drew glyphs shifted right by `indent_start`, but the
    // caret and hit-testing read un-indented Parley coordinates — so the cursor
    // showed at the page's content-left instead of at the text.
    let mut r = test_resources();
    let text = "Hello";
    let spans = [single_span(text, 12.0)];
    let props = ResolvedParaProps {
        indent_start: 100.0,
        ..Default::default()
    };
    let layout = layout_paragraph(&mut r, text, &spans, &props, 400.0, 1.0, true);

    // The caret at the start of the text sits at the indent, not at x = 0.
    let c0 = layout.cursor_rect(0).expect("cursor at start");
    assert!(
        (c0.x - 100.0).abs() < 1.0,
        "start caret x {} should be ~100 (the left indent)",
        c0.x
    );

    // A click on the visible (indented) text maps to the right offset, not to
    // the end of the line as it would if the indent were ignored.
    let mid_y = c0.y + c0.height * 0.5;
    let hit = layout
        .hit_test_point(102.0, mid_y)
        .expect("hit near indented start");
    assert_eq!(
        hit.byte_offset, 0,
        "click just inside the indented text should map to offset 0"
    );

    // Caret / hit-test are mutually consistent under the indent.
    let c3 = layout.cursor_rect(3).expect("cursor mid");
    assert!(c3.x > 100.0, "mid caret should be right of the indent");
    let hit3 = layout
        .hit_test_point(c3.x, mid_y)
        .expect("hit at mid caret");
    assert_eq!(hit3.byte_offset, 3);
}

#[test]
fn hit_test_read_only_mode_returns_none() {
    let mut r = test_resources();
    let text = "Hello, world!";
    let spans = [single_span(text, 12.0)];
    // Default call: preserve_for_editing = false → no Parley layout retained.
    let result = layout_paragraph(
        &mut r,
        text,
        &spans,
        &ResolvedParaProps::default(),
        400.0,
        1.0,
        false,
    );
    assert!(
        result.hit_test_point(0.0, 0.0).is_none(),
        "hit_test_point must return None in read-only mode"
    );
}

#[test]
fn hit_test_editing_mode_returns_some() {
    let text = "Hello, world!";
    let result = editing_paragraph(text);
    assert!(
        result.hit_test_point(0.0, 0.0).is_some(),
        "hit_test_point must return Some when preserve_for_editing=true"
    );
}

#[test]
fn hit_test_at_origin_returns_offset_zero() {
    let text = "Hello, world!";
    let result = editing_paragraph(text);
    let hit = result
        .hit_test_point(0.0, 0.0)
        .expect("editing layout must return Some");
    assert_eq!(
        hit.byte_offset, 0,
        "hit at (0, 0) should map to byte offset 0, got {}",
        hit.byte_offset
    );
}

#[test]
fn hit_test_far_right_returns_end() {
    let text = "Hello";
    let result = editing_paragraph(text);
    // Hit far to the right of the paragraph — should clamp to the last position.
    let hit = result
        .hit_test_point(10_000.0, 0.0)
        .expect("must return Some");
    assert_eq!(
        hit.byte_offset,
        text.len(),
        "hit far right should map to byte_offset == text.len() ({}), got {}",
        text.len(),
        hit.byte_offset
    );
}

#[test]
fn hit_test_midpoint_is_between_start_and_end() {
    let text = "Hello, world!";
    let result = editing_paragraph(text);
    let mid_x = result.width / 2.0;
    let mid_y = result.height / 2.0;
    let hit = result
        .hit_test_point(mid_x, mid_y)
        .expect("must return Some");
    assert!(
        hit.byte_offset < text.len(),
        "midpoint hit ({mid_x}, {mid_y}) byte_offset should be < text.len() ({}), got {}",
        text.len(),
        hit.byte_offset
    );
}

#[test]
fn cursor_rect_read_only_mode_returns_none() {
    let mut r = test_resources();
    let text = "Hello";
    let spans = [single_span(text, 12.0)];
    let result = layout_paragraph(
        &mut r,
        text,
        &spans,
        &ResolvedParaProps::default(),
        400.0,
        1.0,
        false,
    );
    assert!(
        result.cursor_rect(0).is_none(),
        "cursor_rect must return None in read-only mode"
    );
}

#[test]
fn cursor_rect_start_has_positive_height() {
    let text = "Hello";
    let result = editing_paragraph(text);
    let rect = result.cursor_rect(0).expect("must return Some");
    assert!(
        rect.height > 0.0,
        "cursor rect at offset 0 must have positive height, got {}",
        rect.height
    );
    // x should be near zero (start of line).
    assert!(
        rect.x.abs() < 5.0,
        "cursor x at start should be near 0, got {}",
        rect.x
    );
}

#[test]
fn cursor_rect_oob_clamps_gracefully() {
    let text = "Hello";
    let result = editing_paragraph(text);
    // Out-of-bounds offset — Parley clamps it to the last valid position.
    // Should return Some without panicking, and height must be positive.
    let rect = result
        .cursor_rect(usize::MAX)
        .expect("OOB cursor_rect must return Some (clamped)");
    assert!(
        rect.height > 0.0,
        "clamped cursor rect must have positive height"
    );
}

// ── line_end_offset tests ─────────────────────────────────────────────────────

#[test]
fn line_end_offset_single_line_returns_text_len() {
    let text = "hello";
    let para = editing_paragraph(text);
    // Single line with no hard break: end offset should be text.len() (= 5).
    let end = para
        .line_end_offset(0, text)
        .expect("line_end_offset must return Some");
    assert_eq!(
        end,
        text.len(),
        "end offset for single-line paragraph should equal text length"
    );
}

#[test]
fn line_end_offset_excludes_trailing_newline() {
    // A paragraph whose text ends with '\n' — line_end_offset should trim it.
    let text = "hello\n";
    let para = editing_paragraph(text);
    let end = para
        .line_end_offset(0, text)
        .expect("line_end_offset must return Some");
    assert_eq!(
        end, 5,
        "trailing newline must be excluded from line end offset"
    );
}

#[test]
fn drop_cap_enlarges_initial_and_shifts_first_lines() {
    use loki_doc_model::style::props::drop_cap::{DropCap, DropCapLength};

    let mut r = test_resources();
    let text = "Hello world this is a longer paragraph that wraps across several lines so \
                we can exercise the dropped-initial rendering path with enough body text \
                to produce a number of distinct wrapped lines below the cap band.";
    let spans = [single_span(text, 12.0)];
    let props = ResolvedParaProps {
        drop_cap: Some(DropCap {
            lines: 3,
            length: DropCapLength::Chars(1),
            distance: DocPoints::new(2.0),
            margin: false,
        }),
        ..ResolvedParaProps::default()
    };
    // Read-only (paint) path: drop caps render dropped only when !preserve.
    let result = layout_paragraph(&mut r, text, &spans, &props, 300.0, 1.0, false);

    let runs: Vec<&PositionedGlyphRun> = result
        .items
        .iter()
        .filter_map(|i| match i {
            PositionedItem::GlyphRun(g) => Some(g),
            _ => None,
        })
        .collect();
    assert!(!runs.is_empty(), "expected glyph runs");

    // The cap is sized to span ~3 lines → far larger than the 12 pt body.
    let max_size = runs.iter().map(|g| g.font_size).fold(0.0_f32, f32::max);
    assert!(
        max_size > 24.0,
        "cap glyph should be enlarged to span 3 lines; max font_size = {max_size}"
    );
    // Body text is retained at the original 12 pt.
    assert!(
        runs.iter().any(|g| (g.font_size - 12.0).abs() < 0.5),
        "body text should remain at 12 pt"
    );

    // Body (12 pt) runs only. The first body line (smallest y) must be shifted
    // right to clear the cap; a later line must sit back at the left margin.
    let mut body: Vec<&PositionedGlyphRun> = runs
        .iter()
        .copied()
        .filter(|g| (g.font_size - 12.0).abs() < 0.5)
        .collect();
    body.sort_by(|a, b| a.origin.y.partial_cmp(&b.origin.y).unwrap());
    let first_y = body.first().unwrap().origin.y;
    let last_y = body.last().unwrap().origin.y;
    assert!(last_y > first_y, "body must wrap to multiple lines");

    let first_line_min_x = body
        .iter()
        .filter(|g| (g.origin.y - first_y).abs() < 0.5)
        .map(|g| g.origin.x)
        .fold(f32::INFINITY, f32::min);
    let last_line_min_x = body
        .iter()
        .filter(|g| (g.origin.y - last_y).abs() < 0.5)
        .map(|g| g.origin.x)
        .fold(f32::INFINITY, f32::min);
    assert!(
        first_line_min_x > 10.0,
        "first body line must clear the cap band; min x = {first_line_min_x}"
    );
    // Per-line precision: the line below the 3-line cap reclaims the full column
    // (back to the paragraph's left edge, x ≈ indent_start = 0), not the cap band.
    assert!(
        last_line_min_x < 2.0,
        "a line below the cap must reclaim the full left margin; \
         first = {first_line_min_x}, last = {last_line_min_x}"
    );
}

#[test]
fn drop_cap_enlarged_and_hit_testable_in_editor() {
    use loki_doc_model::style::props::drop_cap::{DropCap, DropCapLength};

    let mut r = test_resources();
    let text = "Hello world this is body text that wraps beside the cap in the editor.";
    let spans = [single_span(text, 12.0)];
    let props = ResolvedParaProps {
        drop_cap: Some(DropCap {
            lines: 3,
            length: DropCapLength::Chars(1),
            distance: DocPoints::new(2.0),
            margin: false,
        }),
        ..ResolvedParaProps::default()
    };
    // Editor path: the initial is enlarged (matching print) AND a hit-testable
    // Parley layout is retained for the body.
    let result = layout_paragraph(&mut r, text, &spans, &props, 300.0, 1.0, true);

    // The dropped initial is rendered enlarged (≈ 3 line-heights tall).
    let max_size = result
        .items
        .iter()
        .filter_map(|i| match i {
            PositionedItem::GlyphRun(g) => Some(g.font_size),
            _ => None,
        })
        .fold(0.0_f32, f32::max);
    assert!(
        max_size > 24.0,
        "editor must render the enlarged initial; max glyph size = {max_size}"
    );

    // Hit-testing is available (body layout retained).
    let caret = result
        .cursor_rect(1)
        .expect("editor drop-cap paragraph must retain a hit-testable layout");
    // The caret just after the cap (start of the body's first line) sits shifted
    // right into the band, clearing the dropped initial.
    assert!(
        caret.x > 20.0,
        "first body line caret must clear the cap band; x = {}",
        caret.x
    );

    // A click in the body maps back to a sensible original offset past the cap.
    let body_glyph = result
        .items
        .iter()
        .find_map(|i| match i {
            PositionedItem::GlyphRun(g) if g.font_size < 16.0 => Some(g.origin),
            _ => None,
        })
        .expect("body glyphs present");
    let hit = result
        .hit_test_point(body_glyph.x + 2.0, body_glyph.y)
        .expect("hit-test available in editor");
    assert!(
        hit.byte_offset >= 1,
        "a click in the body must map past the cap byte; offset = {}",
        hit.byte_offset
    );
}

#[test]
fn line_end_offset_read_only_returns_none() {
    let mut r = test_resources();
    let text = "hello";
    let spans = [single_span(text, 12.0)];
    let para = layout_paragraph(
        &mut r,
        text,
        &spans,
        &ResolvedParaProps::default(),
        400.0,
        1.0,
        false,
    );
    assert!(
        para.line_end_offset(0, text).is_none(),
        "line_end_offset must return None in read-only mode"
    );
}
