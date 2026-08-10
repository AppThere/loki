// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 AppThere Loki contributors

use std::sync::{Arc, Mutex};

use loki_doc_model::content::block::Block;
use loki_doc_model::content::inline::Inline;
use loki_doc_model::document::Document;

use crate::editing::state::{DocumentState, compute_seed_layout, publish_seed_layout};
use crate::editing::word_count::count_words;

/// A document whose word count is known by construction rather than by asking the
/// counter — see `an_independently_known_count_is_reproduced`.
fn doc_of_known_words() -> (Document, usize) {
    let mut doc = Document::new();
    // Three paragraphs of 4, 3 and 5 words. Counted by reading, not by running
    // `count_words` and writing down what it said.
    doc.sections[0].blocks = vec![
        Block::Para(vec![Inline::Str("alpha beta gamma delta".into())]),
        Block::Para(vec![Inline::Str("epsilon zeta eta".into())]),
        Block::Para(vec![Inline::Str("theta iota kappa lambda mu".into())]),
    ];
    (doc, 12)
}

/// The defect I-10 reported, stated as a property of the two counters rather than
/// of the UI: publishing a seed layout advances the authoritative generation, and
/// before r19 nothing advanced the reactive mirror, so a mirror captured before and
/// after the publish was unchanged.
///
/// This asserts the *authoritative* half — that the publish really does move
/// `DocumentState::generation`, which is what makes mirroring it meaningful. The
/// mirror write itself needs a Dioxus runtime and is exercised by the editor.
#[test]
fn publishing_a_seed_layout_advances_the_authoritative_generation() {
    let doc_state = Arc::new(Mutex::new(DocumentState::new()));
    let fonts = doc_state
        .lock()
        .expect("lock doc_state")
        .shared_font_resources
        .clone();
    let (doc, _) = doc_of_known_words();

    let before = doc_state.lock().expect("lock doc_state").generation;
    let layout = compute_seed_layout(&fonts, &doc);
    publish_seed_layout(&doc_state, &doc, layout);
    let after = doc_state.lock().expect("lock doc_state").generation;

    assert_ne!(
        before, after,
        "the open path must advance the content generation, or there is nothing \
         for the reactive mirror to carry",
    );
    assert!(
        doc_state.lock().expect("lock doc_state").document.is_some(),
        "and the document must be present once it has, or a count taken at this \
         point would still be counting nothing",
    );
}

/// L08-028 applied to T3.3's acceptance criterion.
///
/// The spec's criterion is "load-time count equals post-no-op-edit count". Both
/// paths call `count_words`, so their agreement is one observation wearing two
/// hats: a counter wrong in the same way twice passes it cleanly. This asserts
/// against a count known by construction instead, which is a different and
/// stronger claim.
#[test]
fn an_independently_known_count_is_reproduced() {
    let (doc, known) = doc_of_known_words();
    assert_eq!(
        count_words(&doc),
        known,
        "the counter must agree with a figure arrived at without it",
    );
}

/// The self-consistency check the spec asked for, kept because it catches a
/// different failure from the one above: a counter that is *correct* on a fresh
/// document but sensitive to having been through the publish path.
#[test]
fn the_count_is_unchanged_by_publishing() {
    let doc_state = Arc::new(Mutex::new(DocumentState::new()));
    let fonts = doc_state
        .lock()
        .expect("lock doc_state")
        .shared_font_resources
        .clone();
    let (doc, known) = doc_of_known_words();

    let before_publish = count_words(&doc);
    let layout = compute_seed_layout(&fonts, &doc);
    publish_seed_layout(&doc_state, &doc, layout);
    let published = doc_state
        .lock()
        .expect("lock doc_state")
        .document
        .clone()
        .expect("publish stored the document");

    assert_eq!(before_publish, known);
    assert_eq!(count_words(&published), known);
}

/// The substituted-fonts defect, stated on the state layer: the chip's source
/// must be per-document, so seeding a document that requests no missing font
/// must *replace* (clear) the substitutions a previous document accumulated —
/// the process-lifetime resolve memo notwithstanding.
#[test]
fn seeding_a_clean_document_replaces_the_substitution_report() {
    use loki_doc_model::content::block::StyledParagraph;
    use loki_doc_model::style::props::CharProps;
    use loki_doc_model::{NodeAttr, StyleId};

    let doc_state = Arc::new(Mutex::new(DocumentState::new()));
    let fonts = doc_state
        .lock()
        .expect("lock doc_state")
        .shared_font_resources
        .clone();

    // Document 1 requests a font that cannot exist.
    let mut doc = Document::new();
    doc.sections[0].blocks = vec![Block::StyledPara(StyledParagraph {
        style_id: Some(StyleId::new("Normal")),
        direct_para_props: None,
        direct_char_props: Some(Box::new(CharProps {
            font_name: Some("Loki Test Nonexistent Face".into()),
            ..CharProps::default()
        })),
        inlines: vec![Inline::Str("text".into())],
        attr: NodeAttr::default(),
    })];
    let layout = compute_seed_layout(&fonts, &doc);
    publish_seed_layout(&doc_state, &doc, layout);
    assert!(
        doc_state
            .lock()
            .expect("lock doc_state")
            .font_substitutions
            .contains_key("Loki Test Nonexistent Face"),
        "the missing font must be reported for the document that requested it"
    );

    // Document 2 requests nothing special: the report must come back empty,
    // even though the shared resolve memo still remembers the missing font.
    let (clean, _) = doc_of_known_words();
    let layout = compute_seed_layout(&fonts, &clean);
    publish_seed_layout(&doc_state, &clean, layout);
    assert!(
        doc_state
            .lock()
            .expect("lock doc_state")
            .font_substitutions
            .is_empty(),
        "a document that requests no missing font must report none — the old \
         behaviour surfaced the previous document's substitutions here"
    );
}
