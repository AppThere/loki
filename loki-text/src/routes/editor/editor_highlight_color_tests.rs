// SPDX-License-Identifier: Apache-2.0

//! Tests for highlight apply/read over a selection, and for the routing that
//! decides whether a picked colour becomes a named `w:highlight` or shading.

use loki_doc_model::content::block::Block;
use loki_doc_model::content::inline::Inline;
use loki_doc_model::document::Document;
use loki_doc_model::loro_bridge::{document_to_loro, loro_to_document};
use loki_doc_model::style::props::char_props::{CharProps, HighlightColor};
use loro::LoroDoc;

use super::{HighlightRoute, apply_highlight, current_highlight, route_for};
use crate::editing::cursor::{CursorState, DocumentPosition};

fn loro_with(text: &str) -> LoroDoc {
    let mut doc = Document::new();
    doc.sections[0].blocks = vec![Block::Para(vec![Inline::Str(text.into())])];
    document_to_loro(&doc).expect("to loro")
}

fn selection(start: usize, end: usize) -> CursorState {
    let mut cs = CursorState::new();
    cs.anchor = Some(DocumentPosition::top_level(0, 0, start));
    cs.focus = Some(DocumentPosition::top_level(0, 0, end));
    cs
}

/// The direct `CharProps` on the first styled run, after a full CRDT round trip.
fn round_tripped_props(loro: &LoroDoc) -> Option<CharProps> {
    let doc = loro_to_document(loro).expect("rebuild");
    let inlines: &[Inline] = match &doc.sections[0].blocks[0] {
        Block::Para(inlines) => inlines,
        Block::StyledPara(sp) => &sp.inlines,
        other => panic!("unexpected block: {other:?}"),
    };
    inlines.iter().find_map(|i| match i {
        Inline::StyledRun(run) => run.direct_props.as_deref().cloned(),
        _ => None,
    })
}

// ── The routing decision ──────────────────────────────────────────────────────

/// A colour that is exactly one of the sixteen goes to the named mark, so the
/// common case still lands in Word's Highlight control.
#[test]
fn an_exact_palette_colour_is_named() {
    assert_eq!(
        route_for(Some("#FFFF00")),
        HighlightRoute::Named(HighlightColor::Yellow)
    );
    assert_eq!(
        route_for(Some("#808000")),
        HighlightRoute::Named(HighlightColor::DarkYellow)
    );
}

/// **And anything else is shading.** Without this the routing test above passes
/// on an implementation that names everything — which is the pre-T5.3 behaviour
/// that wrote a hex into the named mark and applied nothing.
#[test]
fn any_other_colour_is_shading() {
    for hex in ["#C0392B", "#FFFF01", "#123456"] {
        assert_eq!(route_for(Some(hex)), HighlightRoute::Shading, "{hex}");
    }
}

/// **The decision is on the colour, not on where it came from.** A hex typed
/// into the field and the Yellow swatch's own value are the same string by
/// construction now, but the lookup is also case- and `#`-insensitive, so a
/// route that depended on the spelling would show up here.
#[test]
fn the_route_ignores_how_the_colour_was_written() {
    for hex in ["#FFFF00", "#ffff00", "ffff00"] {
        assert_eq!(
            route_for(Some(hex)),
            HighlightRoute::Named(HighlightColor::Yellow),
            "{hex}"
        );
    }
}

/// No colour clears.
#[test]
fn no_colour_clears() {
    assert_eq!(route_for(None), HighlightRoute::Cleared);
}

// ── Applying and reading back ─────────────────────────────────────────────────

#[test]
fn apply_sets_the_highlight_over_the_selection_only() {
    let loro = loro_with("hello world");
    apply_highlight(&loro, &selection(0, 5), Some("#FFFF00")).expect("apply");
    assert_eq!(
        current_highlight(&loro, &selection(2, 2)).as_deref(),
        Some("#FFFF00"),
    );
    assert_eq!(
        current_highlight(&loro, &selection(8, 8)),
        None,
        "untouched outside the selection",
    );
}

/// A custom colour reads back as itself.
#[test]
fn a_custom_colour_applies_and_reads_back() {
    let loro = loro_with("hello world");
    apply_highlight(&loro, &selection(0, 5), Some("#C0392B")).expect("apply");
    assert_eq!(
        current_highlight(&loro, &selection(2, 2)).as_deref(),
        Some("#C0392B"),
    );
}

#[test]
fn clearing_removes_the_direct_highlight() {
    let loro = loro_with("hello");
    apply_highlight(&loro, &selection(0, 5), Some("#00FF00")).expect("apply");
    apply_highlight(&loro, &selection(0, 5), None).expect("clear");
    assert_eq!(current_highlight(&loro, &selection(2, 2)), None);
}

/// **Clearing removes a *custom* highlight too**, which is a different mark. A
/// clear that only touched the named one would leave the shading painting.
#[test]
fn clearing_removes_a_custom_highlight() {
    let loro = loro_with("hello");
    apply_highlight(&loro, &selection(0, 5), Some("#C0392B")).expect("apply");
    apply_highlight(&loro, &selection(0, 5), None).expect("clear");
    assert_eq!(current_highlight(&loro, &selection(2, 2)), None);
    let props = round_tripped_props(&loro).unwrap_or_default();
    assert_eq!(props.background_color, None);
}

/// A named colour reaches `highlight_color`, and **not** `background_color`.
#[test]
fn a_named_colour_lands_on_the_named_property() {
    let loro = loro_with("hello");
    apply_highlight(&loro, &selection(0, 5), Some("#00FFFF")).expect("apply");
    let props = round_tripped_props(&loro).expect("a styled run");
    assert_eq!(props.highlight_color, Some(HighlightColor::Cyan));
    assert_eq!(
        props.background_color, None,
        "a named highlight must not also be shading"
    );
}

/// A custom colour reaches `background_color`, and **not** `highlight_color` —
/// which is what routes it to `w:shd` rather than to a `w:highlight` name that
/// cannot represent it.
#[test]
fn a_custom_colour_lands_on_shading() {
    let loro = loro_with("hello");
    apply_highlight(&loro, &selection(0, 5), Some("#C0392B")).expect("apply");
    let props = round_tripped_props(&loro).expect("a styled run");
    assert_eq!(props.highlight_color, None);
    assert!(
        props.background_color.is_some(),
        "custom highlight must be shading: {props:?}"
    );
}

/// **Switching from custom to named clears the shading**, and the reverse.
/// Both marks paint, and the layout prefers the named one — so a stale mark
/// would make the colour depend on the order the reader picked in, which is the
/// failure that made this test worth writing before the code.
#[test]
fn switching_routes_leaves_no_stale_mark() {
    let loro = loro_with("hello");

    apply_highlight(&loro, &selection(0, 5), Some("#C0392B")).expect("custom");
    apply_highlight(&loro, &selection(0, 5), Some("#FFFF00")).expect("named");
    let props = round_tripped_props(&loro).expect("a styled run");
    assert_eq!(props.highlight_color, Some(HighlightColor::Yellow));
    assert_eq!(
        props.background_color, None,
        "shading survived a named pick"
    );
    assert_eq!(
        current_highlight(&loro, &selection(2, 2)).as_deref(),
        Some("#FFFF00")
    );

    apply_highlight(&loro, &selection(0, 5), Some("#123456")).expect("custom again");
    let props = round_tripped_props(&loro).expect("a styled run");
    assert_eq!(
        props.highlight_color, None,
        "the named highlight survived a custom pick"
    );
    assert!(props.background_color.is_some());
    assert_eq!(
        current_highlight(&loro, &selection(2, 2)).as_deref(),
        Some("#123456")
    );
}

#[test]
fn highlight_round_trips_into_char_props() {
    let loro = loro_with("hello");
    apply_highlight(&loro, &selection(0, 5), Some("#00FFFF")).expect("apply");
    let props = round_tripped_props(&loro).expect("a styled run");
    assert_eq!(props.highlight_color, Some(HighlightColor::Cyan));
}
