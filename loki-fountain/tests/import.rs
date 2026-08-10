// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 AppThere Loki contributors

//! End-to-end Fountain import: a small script with a title page maps onto the
//! screenplay template's style ids with the title page on its own page.

use std::io::Cursor;

use loki_doc_model::content::block::Block;
use loki_doc_model::io::DocumentImport;
use loki_fountain::FountainImport;

const SCRIPT: &str = "\
Title: THE LONG WAIT
Author: A. Person

FADE IN:

INT. WAITING ROOM - DAY

ALEX sits alone. A clock *ticks*.

ALEX
(to no one)
Any minute now.

CUT TO:

EXT. STREET - NIGHT

Rain.
";

fn styles_of(doc: &loki_doc_model::document::Document) -> Vec<(String, bool)> {
    doc.sections[0]
        .blocks
        .iter()
        .map(|b| match b {
            Block::StyledPara(p) => (
                p.style_id
                    .as_ref()
                    .map_or_else(String::new, |s| s.as_str().to_string()),
                p.direct_para_props
                    .as_ref()
                    .and_then(|pp| pp.page_break_before)
                    .unwrap_or(false),
            ),
            other => (format!("{other:?}"), false),
        })
        .collect()
}

#[test]
fn a_script_with_title_page_maps_onto_the_screenplay_styles() {
    let doc = FountainImport::import(Cursor::new(SCRIPT.as_bytes().to_vec()), ())
        .expect("import should succeed");
    let styles = styles_of(&doc);

    let expected: Vec<(&str, bool)> = vec![
        ("TitlePageTitle", false),
        ("TitlePageLine", false),
        // FADE IN: is all-caps ending in a colon but not `TO:`; followed by
        // blank — action per the spec.
        ("Action", true), // first script element starts page 2
        ("SceneHeading", false),
        ("Action", false),
        ("Character", false),
        ("Parenthetical", false),
        ("Dialogue", false),
        ("Transition", false),
        ("SceneHeading", false),
        ("Action", false),
    ];
    assert_eq!(
        styles,
        expected
            .into_iter()
            .map(|(s, b)| (s.to_string(), b))
            .collect::<Vec<_>>()
    );

    // Every referenced id is one the crate promises (and the screenplay
    // template defines) — the mechanical check that keeps the two in step.
    for (style, _) in &styles {
        assert!(
            loki_fountain::REQUIRED_STYLE_IDS.contains(&style.as_str()),
            "unpromised style id {style}"
        );
    }

    // The emphasis survived into inlines.
    let Some(Block::StyledPara(action)) = doc.sections[0].blocks.get(4) else {
        panic!("expected the action para");
    };
    assert!(
        action
            .inlines
            .iter()
            .any(|i| matches!(i, loki_doc_model::content::inline::Inline::Emph(_))),
        "italic *ticks* was lost"
    );
}

#[test]
fn no_title_page_means_no_page_break() {
    let doc = FountainImport::import(Cursor::new(b"INT. HOUSE - DAY\n\nHello.".to_vec()), ())
        .expect("import");
    let styles = styles_of(&doc);
    assert_eq!(styles[0].0, "SceneHeading");
    assert!(!styles[0].1, "no title page, no break");
}
