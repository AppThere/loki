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

// ── Splitting a list item's marker out of its resolved runs ──────────────────

/// The marker, resolved and coalesced exactly as the view does it, then split.
///
/// Goes through `synthesize_list_item_para` rather than prefixing a string here:
/// the whole point of the split is that the marker arrives *inside* the first
/// resolved run, and a hand-built pair of runs would not.
fn split_item(text: &str, marker: &str) -> (Vec<super::Run>, Vec<super::Run>) {
    let para = StyledParagraph {
        style_id: None,
        direct_para_props: None,
        direct_char_props: None,
        inlines: vec![Inline::Str(text.to_string())],
        attr: NodeAttr::default(),
    };
    let para = loki_layout::flow::synthesize_list_item_para(&para, marker, 18.0);
    let mut notes = 0u32;
    let (flat, spans, _, _) = loki_layout::resolve::flatten_paragraph_with_base(
        &para,
        &StyleCatalog::new(),
        &mut notes,
        None,
        loki_layout::RevisionDisplay::default(),
    );
    let runs = coalesce(&flat, &spans, &FamilyMap::new());
    // The precondition, checked: the marker and the text really are one run
    // here, so the split is doing the separating and not the resolver.
    assert_eq!(
        runs.len(),
        1,
        "the marker was already its own run: {runs:?}"
    );
    super::split_runs(runs, marker.len())
}

/// **The marker comes out, the text stays in, and the tab does not survive.**
///
/// The tab is the canvas path's tab stop; here the marker cell's width is, and a
/// tab left in the text paints as a `.notdef` box — which is what the list
/// fixture rendered before the split existed.
#[test]
fn splitting_an_item_leaves_the_marker_without_its_tab() {
    let marker = loki_layout::flow::list_marker(None, 0);
    let (m, body) = split_item("Item one", &marker);
    let marker_text: String = m.iter().map(|(_, _, t)| t.as_str()).collect();
    let body_text: String = body.iter().map(|(_, _, t)| t.as_str()).collect();
    assert_eq!(marker_text, "\u{2022}");
    assert_eq!(body_text, "Item one");
    assert!(!marker_text.contains('\t'), "the tab is still text");
}

/// **The marker carries the paragraph's own character properties.** It is
/// ordinary text on the canvas path, so a marker emitted with default CSS would
/// set in a different face from the item it belongs to.
#[test]
fn the_split_marker_keeps_the_runs_css() {
    let marker = loki_layout::flow::list_marker(Some(&Default::default()), 0);
    let (m, body) = split_item("Item one", &marker);
    assert_eq!(m[0].0, body[0].0, "marker and text disagree on style");
    assert_eq!(m[0].1, body[0].1, "marker and text disagree on features");
}

/// **An offset past the runs takes everything, and one at zero takes nothing.**
/// The inversion: a split that always handed back the first run would pass the
/// tests above and eat a character of every item whose marker resolved apart.
#[test]
fn a_split_offset_outside_the_text_does_not_take_the_wrong_side() {
    let (text, spans) = resolved(vec![Inline::Str("abc".into())]);
    let runs = coalesce(&text, &spans, &FamilyMap::new());

    let (m, body) = super::split_runs(runs.clone(), 0);
    assert!(m.is_empty(), "{m:?}");
    assert_eq!(
        body.iter().map(|(_, _, t)| t.as_str()).collect::<String>(),
        "abc"
    );

    let (m, body) = super::split_runs(runs, 99);
    assert_eq!(
        m.iter().map(|(_, _, t)| t.as_str()).collect::<String>(),
        "abc"
    );
    assert!(body.is_empty(), "{body:?}");
}

/// A split inside a multi-byte character keeps the run whole rather than
/// panicking inside a render. Reachable only if the offset did not come from
/// this text, which is a bug — but a visibly-wrong marker beats a dead
/// application.
#[test]
fn a_split_off_a_character_boundary_is_refused_not_panicked() {
    let (text, spans) = resolved(vec![Inline::Str("\u{2022}x".into())]);
    let runs = coalesce(&text, &spans, &FamilyMap::new());
    let (m, body) = super::split_runs(runs, 1);
    assert!(m.is_empty(), "{m:?}");
    assert_eq!(
        body.iter().map(|(_, _, t)| t.as_str()).collect::<String>(),
        "\u{2022}x"
    );
}

// ── Collecting the families a document asks for ──────────────────────────────

/// A document whose only block is `block`.
fn one_block_doc(block: loki_doc_model::content::block::Block) -> loki_doc_model::Document {
    loki_doc_model::Document {
        meta: Default::default(),
        styles: StyleCatalog::new(),
        sections: vec![loki_doc_model::layout::section::Section {
            blocks: vec![block],
            ..loki_doc_model::layout::section::Section::new()
        }],
        settings: None,
        comments: Vec::new(),
        source: None,
    }
}

/// A one-run paragraph in `family`.
fn para_in(family: &str) -> StyledParagraph {
    StyledParagraph {
        style_id: None,
        direct_para_props: None,
        direct_char_props: Some(Box::new(CharProps {
            font_name: Some(family.to_string()),
            ..Default::default()
        })),
        inlines: vec![Inline::Str("text".into())],
        attr: NodeAttr::default(),
    }
}

/// **A family inside a list is collected**, because a family this walk misses is
/// emitted as the document requested it and resolved by Blitz's fallback policy
/// rather than `loki-layout`'s.
///
/// This is the guard on a measured defect, not a hypothetical: the walk used to
/// look at top-level paragraphs only, so a document of lists asked Blitz for
/// "Arial", got its default sans — about 14 % wider — and every item wrapped one
/// line before the canvas path's (ADR-0017 §5.9).
#[test]
fn a_family_inside_a_list_is_collected() {
    use loki_doc_model::content::block::Block;
    let doc = one_block_doc(Block::BulletList(vec![vec![Block::StyledPara(para_in(
        "Cousine",
    ))]]));
    assert_eq!(super::super::content::requested_families(&doc), ["Cousine"]);
}

/// …and inside a **nested** list, a block quote, and a table cell — every
/// container [`super::super::content::block_el`] recurses into.
#[test]
fn a_family_is_collected_from_every_container_that_renders_one() {
    use loki_doc_model::content::block::Block;

    let nested = Block::BulletList(vec![vec![Block::BulletList(vec![vec![
        Block::StyledPara(para_in("Tinos")),
    ]])]]);
    assert_eq!(
        super::super::content::requested_families(&one_block_doc(nested)),
        ["Tinos"]
    );

    let quoted = Block::BlockQuote(vec![Block::StyledPara(para_in("Carlito"))]);
    assert_eq!(
        super::super::content::requested_families(&one_block_doc(quoted)),
        ["Carlito"]
    );

    // A table cell — the load-bearing arm. Mutating away `collect_families`'
    // `Block::Table` case routes cells to the `_ => continue` catch-all, silently
    // killing table-cell font substitution (the §5.3/§5.9 ~14 %-wide early-wrap
    // defect) with no other test failing. This inverts that arm.
    use loki_doc_model::content::table::Table;
    let mut table = Table::grid(1, 1);
    table.bodies[0].body_rows[0].cells[0].blocks = vec![Block::StyledPara(para_in("Gelasio"))];
    assert_eq!(
        super::super::content::requested_families(&one_block_doc(Block::Table(Box::new(table)))),
        ["Gelasio"]
    );
}

/// The inversion: a document that names no family collects none, so an empty map
/// means "nothing to substitute" and not "the walk missed it".
#[test]
fn a_document_that_names_no_family_collects_none() {
    use loki_doc_model::content::block::Block;
    let plain = StyledParagraph {
        style_id: None,
        direct_para_props: None,
        direct_char_props: None,
        inlines: vec![Inline::Str("text".into())],
        attr: NodeAttr::default(),
    };
    let doc = one_block_doc(Block::BulletList(vec![vec![Block::StyledPara(plain)]]));
    assert!(super::super::content::requested_families(&doc).is_empty());
}
