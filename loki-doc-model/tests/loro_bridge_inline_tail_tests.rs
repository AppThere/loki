// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 AppThere Loki contributors

//! Loro bridge round-trip tests for the inline "tail" fixes: non-Rgb character
//! colors (Theme/Cmyk), comment/bookmark anchors, and quote-type / span-attr
//! range marks — all previously dropped or collapsed by the bridge.

use loki_doc_model::content::annotation::comment::{CommentRef, CommentRefKind};
use loki_doc_model::content::attr::NodeAttr;
use loki_doc_model::content::block::Block;
use loki_doc_model::content::inline::{BookmarkKind, Inline, QuoteType, StyledRun};
use loki_doc_model::document::Document;
use loki_doc_model::loro_bridge::{document_to_loro, loro_to_document};
use loki_doc_model::style::props::char_props::CharProps;
use loki_primitives::color::{CmykColor, DocumentColor, ThemeColorSlot};

fn round_trip_inlines(inlines: Vec<Inline>) -> Vec<Inline> {
    let mut doc = Document::new();
    doc.sections[0].blocks.push(Block::Para(inlines));
    let loro = document_to_loro(&doc).expect("document_to_loro must succeed");
    let recovered = loro_to_document(&loro).expect("loro_to_document must succeed");
    match &recovered.sections[0].blocks[0] {
        Block::Para(inlines) => inlines.clone(),
        other => panic!("expected Para, got {other:?}"),
    }
}

fn styled_run(text: &str, props: CharProps) -> Inline {
    Inline::StyledRun(StyledRun {
        style_id: None,
        direct_props: Some(Box::new(props)),
        content: vec![Inline::Str(text.into())],
        attr: NodeAttr::default(),
    })
}

fn recovered_color(inlines: &[Inline]) -> Option<DocumentColor> {
    inlines.iter().find_map(|i| {
        if let Inline::StyledRun(run) = i {
            run.direct_props.as_ref().and_then(|p| p.color.clone())
        } else {
            None
        }
    })
}

// ── Non-Rgb character colors ──────────────────────────────────────────────────

#[test]
fn theme_char_color_survives_roundtrip() {
    let color = DocumentColor::Theme {
        slot: ThemeColorSlot::Accent2,
        tint: -0.5,
    };
    let props = CharProps {
        color: Some(color.clone()),
        ..Default::default()
    };
    let recovered = round_trip_inlines(vec![styled_run("themed", props)]);
    assert_eq!(recovered_color(&recovered), Some(color));
}

#[test]
fn cmyk_char_color_survives_roundtrip() {
    let color = DocumentColor::Cmyk(CmykColor::new(0.9, 0.1, 0.0, 0.25));
    let props = CharProps {
        color: Some(color.clone()),
        ..Default::default()
    };
    let recovered = round_trip_inlines(vec![styled_run("print", props)]);
    assert_eq!(recovered_color(&recovered), Some(color));
}

// ── Comment / bookmark anchors ────────────────────────────────────────────────

#[test]
fn comment_anchors_survive_roundtrip() {
    let start = Inline::Comment(CommentRef::new("c1", CommentRefKind::Start));
    let end = Inline::Comment(CommentRef::new("c1", CommentRefKind::End));
    let recovered = round_trip_inlines(vec![
        Inline::Str("before ".into()),
        start.clone(),
        Inline::Str("commented".into()),
        end.clone(),
        Inline::Str(" after".into()),
    ]);
    let anchors: Vec<_> = recovered
        .iter()
        .filter(|i| matches!(i, Inline::Comment(_)))
        .collect();
    assert_eq!(anchors.len(), 2, "both comment anchors must survive");
    assert_eq!(anchors[0], &start);
    assert_eq!(anchors[1], &end);
    // The anchors must stay positioned between the text runs.
    let text: String = recovered
        .iter()
        .map(|i| match i {
            Inline::Str(s) => s.as_str(),
            _ => "|",
        })
        .collect();
    assert_eq!(text, "before |commented| after");
}

#[test]
fn bookmark_markers_survive_roundtrip() {
    let start = Inline::Bookmark(BookmarkKind::Start, "bm-1".into());
    let end = Inline::Bookmark(BookmarkKind::End, "bm-1".into());
    let recovered = round_trip_inlines(vec![
        start.clone(),
        Inline::Str("marked".into()),
        end.clone(),
    ]);
    let marks: Vec<_> = recovered
        .iter()
        .filter(|i| matches!(i, Inline::Bookmark(_, _)))
        .collect();
    assert_eq!(marks.len(), 2, "both bookmark markers must survive");
    assert_eq!(marks[0], &start);
    assert_eq!(marks[1], &end);
}

// ── Quote type / span attrs ───────────────────────────────────────────────────

#[test]
fn quoted_text_keeps_quote_type() {
    for quote_type in [QuoteType::SingleQuote, QuoteType::DoubleQuote] {
        let quoted = Inline::Quoted(quote_type, vec![Inline::Str("quoted".into())]);
        let recovered = round_trip_inlines(vec![quoted.clone()]);
        assert_eq!(
            recovered,
            vec![quoted],
            "{quote_type:?} must survive Loro round-trip"
        );
    }
}

#[test]
fn span_keeps_node_attr() {
    let mut attr = NodeAttr::default();
    attr.id = Some("term-3".into());
    attr.classes.push("glossary-term".into());
    attr.kv.push(("data-ref".into(), "g3".into()));
    let span = Inline::Span(attr, vec![Inline::Str("spanned".into())]);
    let recovered = round_trip_inlines(vec![span.clone()]);
    assert_eq!(recovered, vec![span]);
}

#[test]
fn quoted_span_nesting_order_is_preserved() {
    let mut attr = NodeAttr::default();
    attr.classes.push("inner".into());
    let quoted_span = Inline::Quoted(
        QuoteType::DoubleQuote,
        vec![Inline::Span(attr, vec![Inline::Str("both".into())])],
    );
    let recovered = round_trip_inlines(vec![quoted_span.clone()]);
    assert_eq!(recovered, vec![quoted_span]);
}
