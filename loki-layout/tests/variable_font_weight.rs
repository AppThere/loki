// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 AppThere Loki contributors

//! Regression lock: a bold run in a **variable** font must carry the shaped
//! instance's normalized variation coordinates on its `PositionedGlyphRun`.
//!
//! The bundled Arimo (Arial substitute) is a `wght` variable font. Parley
//! shapes a bold run at `wght=700` (bold advances), but the painters render
//! whatever normalized coordinates the run carries — so if that vector is
//! empty/default they draw the *regular* master with bold advances, i.e. "bold
//! Arial looks wide but not bold". This pins the fix: the bold instance carries
//! a non-zero coordinate and differs from the regular instance.

use loki_layout::{
    FontResources, LayoutColor, PositionedItem, ResolvedParaProps, StyleSpan, layout_paragraph,
};

fn arimo_span(text: &str, weight: u16, bold: bool) -> StyleSpan {
    StyleSpan {
        range: 0..text.len(),
        font_name: Some("Arimo".into()),
        font_size: 24.0,
        bold,
        weight,
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

/// The first glyph run's normalized variation coordinates for `text` at the
/// given weight.
fn coords(text: &str, weight: u16, bold: bool) -> Vec<i16> {
    let mut resources = FontResources::new();
    for blob in loki_fonts::fallback_font_blobs() {
        resources.register_font(blob.to_vec());
    }
    let para = layout_paragraph(
        &mut resources,
        text,
        &[arimo_span(text, weight, bold)],
        &ResolvedParaProps::default(),
        1000.0,
        1.0,
        false,
    );
    for item in &para.items {
        if let PositionedItem::GlyphRun(run) = item {
            return run.normalized_coords.clone();
        }
    }
    panic!("no glyph run produced for {text:?}");
}

#[test]
fn bold_variable_font_carries_nonzero_weight_coord() {
    let bold = coords("Weight", 700, true);
    assert!(
        bold.iter().any(|&c| c != 0),
        "bold Arimo must carry a non-zero wght variation coord (else the \
         painter draws the regular master with bold advances); got {bold:?}"
    );
}

#[test]
fn bold_and_regular_variable_instances_differ() {
    let regular = coords("Weight", 400, false);
    let bold = coords("Weight", 700, true);
    assert_ne!(
        regular, bold,
        "the 400 and 700 instances of a variable font must differ; \
         regular={regular:?} bold={bold:?}"
    );
}
