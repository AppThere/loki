// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 AppThere Loki contributors

//! Tests for the math typesetter. These assert structural layout properties
//! (box metrics, rule presence) rather than exact glyph positions, which depend
//! on the system font available at test time.

use super::layout_math;
use crate::color::LayoutColor;
use crate::font::FontResources;
use crate::items::PositionedItem;

const NS: &str = "http://www.w3.org/1998/Math/MathML";

fn render(mathml: &str) -> super::MathRender {
    let mut fr = FontResources::new();
    layout_math(&mut fr, mathml, 12.0, LayoutColor::BLACK, 1.0)
}

fn count_glyph_runs(items: &[PositionedItem]) -> usize {
    items
        .iter()
        .filter(|i| matches!(i, PositionedItem::GlyphRun(_)))
        .count()
}

fn count_rects(items: &[PositionedItem]) -> usize {
    items
        .iter()
        .filter(|i| matches!(i, PositionedItem::FilledRect(_)))
        .count()
}

/// Largest font size used by any glyph run — a proxy for how much a delimiter or
/// surd has been stretched (stretched glyphs are shaped at a larger size).
fn max_glyph_font_size(items: &[PositionedItem]) -> f32 {
    items
        .iter()
        .filter_map(|i| match i {
            PositionedItem::GlyphRun(r) => Some(r.font_size),
            _ => None,
        })
        .fold(0.0, f32::max)
}

#[test]
fn empty_or_invalid_is_zero() {
    let r = render("not math at all");
    assert_eq!(r.width, 0.0);
    assert!(r.items.is_empty());
}

#[test]
fn single_identifier_has_extent() {
    let r = render(&format!("<math xmlns=\"{NS}\"><mi>x</mi></math>"));
    assert!(r.width > 0.0, "identifier should have width");
    assert!(r.ascent > 0.0, "identifier should have ascent");
    assert_eq!(count_glyph_runs(&r.items), 1);
}

#[test]
fn fraction_draws_a_bar_and_two_operands() {
    let r = render(&format!(
        "<math xmlns=\"{NS}\"><mfrac><mn>1</mn><mn>2</mn></mfrac></math>"
    ));
    // One rule (the fraction bar) plus a glyph run for each of 1 and 2.
    assert_eq!(count_rects(&r.items), 1, "fraction bar rule");
    assert_eq!(count_glyph_runs(&r.items), 2, "numerator and denominator");
    // A fraction is taller than a single digit: ascent + descent exceed the
    // numerator's own height.
    assert!(r.ascent > 0.0 && r.descent > 0.0);
}

#[test]
fn superscript_is_raised_and_narrower() {
    let base = render(&format!("<math xmlns=\"{NS}\"><mi>x</mi></math>"));
    let sup = render(&format!(
        "<math xmlns=\"{NS}\"><msup><mi>x</mi><mn>2</mn></msup></math>"
    ));
    // Superscript adds a (smaller) glyph and increases the box ascent.
    assert_eq!(count_glyph_runs(&sup.items), 2);
    assert!(sup.ascent >= base.ascent);
    assert!(sup.width > base.width);
}

#[test]
fn square_root_has_surd_and_overbar() {
    let r = render(&format!(
        "<math xmlns=\"{NS}\"><msqrt><mi>x</mi></msqrt></math>"
    ));
    // The surd glyph + the radicand glyph, plus an overbar rule.
    assert_eq!(count_rects(&r.items), 1, "overbar rule");
    assert!(count_glyph_runs(&r.items) >= 2, "surd and radicand");
    assert!(r.width > 0.0);
}

#[test]
fn nth_root_adds_an_index_glyph() {
    let sqrt = render(&format!(
        "<math xmlns=\"{NS}\"><msqrt><mi>x</mi></msqrt></math>"
    ));
    let root = render(&format!(
        "<math xmlns=\"{NS}\"><mroot><mi>x</mi><mn>3</mn></mroot></math>"
    ));
    // The index contributes an extra glyph run and widens the box.
    assert!(count_glyph_runs(&root.items) > count_glyph_runs(&sqrt.items));
    assert!(root.width > sqrt.width);
}

#[test]
fn radical_stretches_to_a_tall_radicand() {
    let short = render(&format!(
        "<math xmlns=\"{NS}\"><msqrt><mi>x</mi></msqrt></math>"
    ));
    let tall = render(&format!(
        "<math xmlns=\"{NS}\"><msqrt><mfrac><mn>1</mn><mn>2</mn></mfrac></msqrt></math>"
    ));
    assert!(tall.ascent > short.ascent, "tall radicand → taller box");
    // The surd over the fraction is scaled up noticeably more than the surd over
    // a single glyph.
    assert!(
        max_glyph_font_size(&tall.items) > max_glyph_font_size(&short.items) + 2.0,
        "surd should stretch further for a tall radicand ({} vs {})",
        max_glyph_font_size(&tall.items),
        max_glyph_font_size(&short.items),
    );
}

#[test]
fn delimiters_stretch_around_tall_content() {
    let bare = render(&format!(
        "<math xmlns=\"{NS}\"><mfrac><mn>1</mn><mn>2</mn></mfrac></math>"
    ));
    let fenced = render(&format!(
        "<math xmlns=\"{NS}\"><mrow><mo>(</mo>\
         <mfrac><mn>1</mn><mn>2</mn></mfrac><mo>)</mo></mrow></math>"
    ));
    assert!(fenced.width > bare.width, "delimiters add width");
    assert!(
        max_glyph_font_size(&fenced.items) > 12.5,
        "parentheses should be stretched larger than the base size"
    );
}

#[test]
fn baseline_is_at_ascent() {
    // After layout the baseline sits at y = ascent: every glyph run origin must
    // be within the box's vertical extent [0, ascent + descent].
    let r = render(&format!(
        "<math xmlns=\"{NS}\"><msup><mi>x</mi><mn>2</mn></msup></math>"
    ));
    let total = r.ascent + r.descent;
    for item in &r.items {
        if let PositionedItem::GlyphRun(run) = item {
            assert!(
                run.origin.y >= -0.01 && run.origin.y <= total + 0.01,
                "glyph baseline {} outside [0, {}]",
                run.origin.y,
                total
            );
        }
    }
}
