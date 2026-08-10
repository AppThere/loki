// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 AppThere Loki contributors

//! Hollywood screenplay template (US-standard Courier formatting).
//!
//! Scene headings, character cues, and transitions are conventionally typed in
//! uppercase; the sample content uses literal capitals (rather than an all-caps
//! style property, which the DOCX round-trip does not preserve) so the bundled
//! asset re-imports faithfully.

use loki_doc_model::document::Document;
use loki_doc_model::style::props::para_props::ParagraphAlignment::Right;

use loki_doc_model::style::props::para_props::ParagraphAlignment::Center;

use crate::helpers::{Char, Para, assemble, inches, letter_layout, p, p_page_break, style};

// Courier Prime (bundled, loki-fonts): drawn as a Courier replacement with
// Courier's metrics — the screenplay face, with no substitution chip.
const MONO: &str = "Courier Prime";

fn courier() -> Char {
    Char {
        font: Some(MONO),
        size: Some(12.0),
        ..Default::default()
    }
}

/// Builds the screenplay template. Margins follow the US standard: 1.5-inch
/// left, 1-inch elsewhere. Element indents are measured from the text margin.
pub(crate) fn build() -> Document {
    let styles = vec![
        // Action / Normal: full measure.
        style(
            "Normal",
            "Action",
            None,
            Some("Normal"),
            &courier(),
            &Para {
                space_after: Some(12.0),
                ..Default::default()
            },
        ),
        // Scene heading (slug line): bold, space before.
        style(
            "SceneHeading",
            "Scene Heading",
            Some("Normal"),
            Some("Normal"),
            &Char {
                font: Some(MONO),
                size: Some(12.0),
                bold: true,
                ..Default::default()
            },
            &Para {
                space_before: Some(12.0),
                space_after: Some(12.0),
                outline: Some(1),
                ..Default::default()
            },
        ),
        // Character cue: indented ~2.2in from the text margin.
        style(
            "Character",
            "Character",
            Some("Normal"),
            Some("Dialogue"),
            &courier(),
            &Para {
                indent_left: Some(158.0),
                ..Default::default()
            },
        ),
        // Parenthetical: indented ~1.6in.
        style(
            "Parenthetical",
            "Parenthetical",
            Some("Normal"),
            Some("Dialogue"),
            &courier(),
            &Para {
                indent_left: Some(115.0),
                ..Default::default()
            },
        ),
        // Dialogue: indented 1in, right inset ~1.5in. Next style is Action —
        // after a speech the writer describes what happens, not more speech.
        style(
            "Dialogue",
            "Dialogue",
            Some("Normal"),
            Some("Normal"),
            &courier(),
            &Para {
                indent_left: Some(72.0),
                space_after: Some(12.0),
                ..Default::default()
            },
        ),
        // Title page: centered lines, Courier 12 like everything else. The
        // title sits ~3.5in down (industry layout); the lines that follow
        // ("written by", the author) follow each other.
        style(
            "TitlePageTitle",
            "Title Page Title",
            Some("Normal"),
            Some("TitlePageLine"),
            &courier(),
            &Para {
                align: Some(Center),
                space_before: Some(252.0),
                space_after: Some(24.0),
                ..Default::default()
            },
        ),
        style(
            "TitlePageLine",
            "Title Page Line",
            Some("Normal"),
            Some("TitlePageLine"),
            &courier(),
            &Para {
                align: Some(Center),
                space_after: Some(24.0),
                ..Default::default()
            },
        ),
        // Transition: right-aligned.
        style(
            "Transition",
            "Transition",
            Some("Normal"),
            Some("SceneHeading"),
            &courier(),
            &Para {
                align: Some(Right),
                space_before: Some(12.0),
                space_after: Some(12.0),
                ..Default::default()
            },
        ),
    ];

    let body = vec![
        // Title page (conventionally uppercase, like the cues below).
        p("TitlePageTitle", "YOUR SCREENPLAY TITLE"),
        p("TitlePageLine", "written by"),
        p("TitlePageLine", "Your Name"),
        // The direct page break ends the title page; the script starts here.
        p_page_break("SceneHeading", "INT. COFFEE SHOP - DAY"),
        p(
            "Normal",
            "A quiet corner cafe. RAIN streaks the window. ALEX sits alone, \
           staring at a laptop that refuses to cooperate.",
        ),
        p("Character", "ALEX"),
        p("Parenthetical", "(under their breath)"),
        p("Dialogue", "Come on. Just compile. One time."),
        p(
            "Normal",
            "The screen flickers. A cursor blinks, patient and unbothered.",
        ),
        p("Transition", "CUT TO:"),
    ];

    // US standard: 1.5-inch left margin, 1 inch elsewhere.
    let mut layout = letter_layout(1.0);
    layout.margins.left = inches(1.5);
    assemble("Screenplay", layout, styles, body)
}
