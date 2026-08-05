// SPDX-License-Identifier: Apache-2.0

//! Tests for span coalescing in the DOM reflow view (ADR-0017 §5.4–§5.5).
//!
//! Coalescing is an **economy** — a run split with no formatting behind it is
//! nodes for nothing — and not a correctness fix; the line-break disagreement it
//! was first written for turned out to be missing `white-space: pre-wrap`. What
//! these pin is that the economy is exact: the same characters, in the same
//! order, under the same CSS.

use super::FamilyMap;
use super::coalesce;
use loki_doc_model::content::attr::NodeAttr;
use loki_doc_model::content::block::StyledParagraph;
use loki_doc_model::content::inline::{Inline, StyledRun};
use loki_doc_model::style::catalog::StyleCatalog;
use loki_doc_model::style::props::char_props::CharProps;

/// Runs the real resolver, so the spans under test are the spans the view emits.
/// Building `StyleSpan`s by hand would be a second statement of what "resolved"
/// means and would keep passing after the resolver changed.
fn resolved(inlines: Vec<Inline>) -> (String, Vec<loki_layout::para::StyleSpan>) {
    let para = StyledParagraph {
        style_id: None,
        direct_para_props: None,
        direct_char_props: None,
        inlines,
        attr: NodeAttr::default(),
    };
    let mut notes = 0u32;
    let (text, spans, _, _) = loki_layout::resolve::flatten_paragraph_with_base(
        &para,
        &StyleCatalog::new(),
        &mut notes,
        None,
        loki_layout::RevisionDisplay::default(),
    );
    (text, spans)
}

/// A run with `props`, wrapping `text`.
fn run(text: &str, props: CharProps) -> Inline {
    Inline::StyledRun(StyledRun {
        style_id: None,
        direct_props: Some(Box::new(props)),
        content: vec![Inline::Str(text.to_string())],
        attr: NodeAttr::default(),
    })
}

/// **Three runs that resolve alike become one span**, carrying all the text.
///
/// The precondition is checked rather than assumed: the resolver really does
/// hand back more than one span here, so the test is about the joining and not
/// about a paragraph that was never split.
#[test]
fn adjacent_runs_that_resolve_alike_become_one_span() {
    let (text, spans) = resolved(vec![
        Inline::Str("one ".into()),
        run("two ", CharProps::default()),
        Inline::Str("three".into()),
    ]);
    assert!(
        spans.len() > 1,
        "the resolver did not split this paragraph, so nothing is being joined"
    );

    let merged = coalesce(&text, &spans, &FamilyMap::new());
    assert_eq!(merged.len(), 1, "runs that resolve alike stayed separate");
    assert_eq!(merged[0].2, "one two three");
}

/// **A run that genuinely differs keeps its own span.** The inversion of the
/// test above: a coalescer that joined everything would pass that one and paint
/// the bold words unbolded.
#[test]
fn a_run_that_differs_keeps_its_own_span() {
    let bold = CharProps {
        bold: Some(true),
        font_weight: Some(700),
        ..Default::default()
    };
    let (text, spans) = resolved(vec![
        Inline::Str("one ".into()),
        run("two ", bold),
        Inline::Str("three".into()),
    ]);

    let merged = coalesce(&text, &spans, &FamilyMap::new());
    assert_eq!(
        merged.len(),
        3,
        "the differing run was absorbed: {merged:?}"
    );
    assert_eq!(merged[1].2, "two ");
    assert!(
        merged[1].0.contains("font-weight: 700"),
        "{:?}",
        merged[1].0
    );
    assert!(!merged[0].0.contains("font-weight: 700"));
}

/// **Non-adjacent equal runs are not joined across the one between them.**
/// Joining by CSS alone — rather than by CSS *and* adjacency — would reorder the
/// paragraph's text, which no line-count comparison would ever notice.
#[test]
fn equal_runs_on_either_side_of_a_different_one_stay_apart() {
    let italic = CharProps {
        italic: Some(true),
        ..Default::default()
    };
    let (text, spans) = resolved(vec![
        Inline::Str("one ".into()),
        run("two ", italic),
        Inline::Str("three".into()),
    ]);

    let merged = coalesce(&text, &spans, &FamilyMap::new());
    assert_eq!(merged.len(), 3);
    let joined: String = merged.iter().map(|(_, _, t)| t.as_str()).collect();
    assert_eq!(joined, text, "coalescing changed the paragraph's text");
}

/// The joined text is the whole paragraph, in order — for the all-alike case as
/// well, where a dropped slice would look like a shorter document rather than
/// like a bug.
#[test]
fn coalescing_preserves_every_character_in_order() {
    let (text, spans) = resolved(vec![
        Inline::Str("alpha ".into()),
        run("beta ", CharProps::default()),
        run("gamma", CharProps::default()),
    ]);
    let merged = coalesce(&text, &spans, &FamilyMap::new());
    let joined: String = merged.iter().map(|(_, _, t)| t.as_str()).collect();
    assert_eq!(joined, text);
}
