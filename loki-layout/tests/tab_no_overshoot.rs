// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 AppThere Loki contributors

//! Regression: a `\t` must not add its own glyph advance on top of the tab
//! box, or the following content overshoots the stop.
//!
//! The bullet-list case: a marker `●\t` at a 0.25in hanging indent with the
//! text indent at 0.5in. The tab is realised by an inline box that advances the
//! pen to the stop; the `\t` itself is excluded from the shaped text (in a font
//! without a tab glyph — e.g. Arimo — it would otherwise shape to a `.notdef`
//! whose ~8pt advance pushed the text to ~0.61in, visibly past Word's 0.5in).

use loki_layout::{
    FontResources, LayoutColor, PositionedItem, ResolvedParaProps, StyleSpan, layout_paragraph,
};

fn span(text: &str) -> StyleSpan {
    StyleSpan {
        range: 0..text.len(),
        font_name: Some("Arimo".into()),
        font_size: 11.0,
        bold: false,
        weight: 400,
        italic: false,
        color: LayoutColor::BLACK,
        underline: None,
        strikethrough: None,
        line_height: None,
        vertical_align: None,
        highlight_color: None,
        character_border: None,
        letter_spacing: None,
        font_variant: None,
        word_spacing: None,
        shadow: false,
        emboss: false,
        imprint: false,
        link_url: None,
        math: None,
        scale: None,
        kerning: None,
        baseline_shift: None,
        language: None,
    }
}

/// Absolute x (pt) of the first drawable (non-`.notdef`) glyph.
fn first_text_x(text: &str, indent_start: f32, indent_hanging: f32) -> f32 {
    let mut r = FontResources::new();
    for b in loki_fonts::fallback_font_blobs() {
        r.register_font(b.to_vec());
    }
    let props = ResolvedParaProps {
        indent_start,
        indent_hanging,
        ..Default::default()
    };
    let para = layout_paragraph(&mut r, text, &[span(text)], &props, 468.0, 1.0, false);
    // Skip the marker run (origin at the hanging position, `indent_start -
    // indent_hanging`); the text run's origin sits at `indent_start`.
    for item in &para.items {
        if let PositionedItem::GlyphRun(g) = item
            && g.origin.x >= indent_start - 1.0
            && let Some(gl) = g.glyphs.iter().find(|gl| gl.id != 0)
        {
            return g.origin.x + gl.x;
        }
    }
    panic!("no text glyph run at the indent produced");
}

#[test]
fn list_marker_tab_lands_text_on_the_indent_not_past_it() {
    // 0.5in indent = 36pt, 0.25in hanging = 18pt.
    let x = first_text_x("\u{25CF}\tSource Nodes", 36.0, 18.0);
    assert!(
        (x - 36.0).abs() < 1.5,
        "list text must land on the 36pt (0.5in) indent, not overshoot; got {x}pt"
    );
}
