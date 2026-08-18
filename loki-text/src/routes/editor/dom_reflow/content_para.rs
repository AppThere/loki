// SPDX-License-Identifier: Apache-2.0

//! A styled paragraph and its runs, for the DOM reflow view.
//!
//! Split from [`super::content`] for the 300-line ceiling: the block dispatch
//! and the inline marks stay there, and the half that goes through
//! `loki_layout`'s resolver lives here.

use dioxus::prelude::*;
use loki_doc_model::style::catalog::StyleCatalog;

use super::content::FamilyMap;
use super::style::{
    HANGING_BODY_CSS, hanging_body_props, hanging_marker_css, hanging_row_css, resolved_para_css,
    span_css,
};

/// One emitted run: its CSS, its OpenType feature list, and its text.
pub(super) type Run = (String, &'static str, String);

/// A styled paragraph, rendered from **resolved** properties.
///
/// Both halves come from `loki_layout` — `resolve_para_props` for the paragraph
/// and `flatten_paragraph_with_base` for the runs — so this path and the canvas
/// path resolve the catalog through the same code rather than through two
/// implementations of the same cascade. That is what makes a comparison between
/// them a comparison of *rendering*.
///
/// The flattened text is sliced by each span's byte range, which is the range
/// the same function handed the shaper.
///
/// # Adjacent runs that resolve alike are emitted as one span
///
/// Documents are full of run splits that carry no formatting meaning — a DOCX
/// run boundary survives spell-check state and revision ids — and emitting a
/// `<span>` for each is nodes for nothing.
///
/// **This is an economy, not a fix**, and the distinction is on the record
/// because it was got wrong once. It was added when a three-run paragraph with
/// *identical* properties broke differently from the canvas path at 4 of 12
/// widths; that disagreement turned out to be the missing
/// `white-space: pre-wrap` in [`super::style::resolved_para_css`] (ADR-0017
/// §5.5), not the boundary. With the whitespace fixed, the same paragraph agrees
/// at every swept width **with coalescing switched off** — a redundant boundary
/// is measurably harmless.
pub(super) fn styled_para_el(
    para: &loki_doc_model::content::block::StyledParagraph,
    catalog: &StyleCatalog,
    families: &FamilyMap,
) -> Element {
    para_el(para, catalog, families, None)
}

/// A list item's first paragraph, with its marker in the hanging space.
///
/// `marker_bytes` is the length of the marker
/// `loki_layout::flow::synthesize_list_item_para` prefixed into `para`. The
/// marker is split back out of the **resolved** runs rather than rendered from
/// the marker string, so it carries the paragraph's own character properties —
/// which is what the canvas path shapes it with, it being ordinary text there.
///
/// See [`super::style::hanging_row_css`] for why the hanging indent is a box on
/// this path and a `text-indent` on no path at all.
pub(super) fn hanging_marker_para_el(
    para: &loki_doc_model::content::block::StyledParagraph,
    catalog: &StyleCatalog,
    families: &FamilyMap,
    marker_bytes: usize,
) -> Element {
    para_el(para, catalog, families, Some(marker_bytes))
}

/// Both of the above: one paragraph, optionally splitting a leading marker out
/// into the hanging space.
fn para_el(
    para: &loki_doc_model::content::block::StyledParagraph,
    catalog: &StyleCatalog,
    families: &FamilyMap,
    marker_bytes: Option<usize>,
) -> Element {
    let resolved = loki_layout::resolve::resolve_para_props(para, catalog);
    // The note counter is local and discarded: this view does not render
    // footnotes, and advancing a shared counter for numbers nothing shows would
    // renumber the notes the canvas path does render.
    let mut notes = 0u32;
    // The paragraph-mark size (5th element) is dropped: this view lays out
    // through the DOM, not Parley, so an empty paragraph's height comes from
    // CSS rather than `ResolvedParaProps::default_font_size`.
    let (text, spans, images, _notes, _para_mark_size) =
        loki_layout::resolve::flatten_paragraph_with_base(
            para,
            catalog,
            &mut notes,
            None,
            loki_layout::RevisionDisplay::default(),
        );
    let runs = coalesce(&text, &spans, families);
    let Some(at) = marker_bytes else {
        return rsx! {
            // Images first, as a block-level prefix — the same placement
            // `stack_block_images` gives them on the canvas path, where Parley
            // has no inline image box either (`TODO(inline-image-flow)`).
            { super::image::images_el(&images) }
            p { style: resolved_para_css(&resolved), { runs_el(runs) } }
        };
    };
    let (marker, body) = split_runs(runs, at);
    rsx! {
        { super::image::images_el(&images) }
        div {
            style: hanging_row_css(&resolved),
            div { style: hanging_marker_css(&resolved), { runs_el(marker) } }
            div {
                style: HANGING_BODY_CSS,
                p { style: resolved_para_css(&hanging_body_props(&resolved)), { runs_el(body) } }
            }
        }
    }
}

/// The runs of one paragraph, or of one marker cell.
fn runs_el(runs: Vec<Run>) -> Element {
    rsx! {
        for (i, (css, features, slice)) in runs.into_iter().enumerate() {
            {
                rsx! {
                    span {
                        key: "{i}",
                        style: "{css}",
                        // Not a CSS property: this build's Stylo has no
                        // `font-kerning`. See `style::span_font_features`.
                        "data-font-features": "{features}",
                        "{slice}"
                    }
                }
            }
        }
    }
}

/// Splits coalesced runs at byte `at` of the flattened text.
///
/// The marker was prefixed into the paragraph's inlines, so after resolution it
/// sits at the front of the first run — usually *inside* it, since it resolves
/// to the same properties as the text it precedes and [`coalesce`] then joins
/// the two.
///
/// # The trailing tab does not survive the split
///
/// On the canvas path the marker's tab is what carries the text to the hanging
/// indent. Here the marker cell's width does that, so the tab is not text: left
/// in, it paints as a `.notdef` box, which is what the list fixture rendered
/// before this existed.
pub(super) fn split_runs(runs: Vec<Run>, at: usize) -> (Vec<Run>, Vec<Run>) {
    let (mut marker, mut body) = (Vec::new(), Vec::new());
    let mut consumed = 0usize;
    for (css, features, text) in runs {
        let remaining = at.saturating_sub(consumed);
        consumed += text.len();
        if remaining == 0 || !text.is_char_boundary(remaining.min(text.len())) {
            // Past the marker, or an offset that is not a boundary of this text
            // — which would mean it did not come from it. The run goes to the
            // body whole rather than panicking mid-render: a marker rendered
            // inline is visibly wrong, and a dead application is not.
            body.push((css, features, text));
        } else if remaining >= text.len() {
            marker.push((css, features, text));
        } else {
            let (head, tail) = text.split_at(remaining);
            marker.push((css.clone(), features, head.to_string()));
            body.push((css, features, tail.to_string()));
        }
    }
    for run in &mut marker {
        run.2 = run.2.trim_end_matches('\t').to_string();
    }
    (marker, body)
}

/// Resolved spans as `(css, text)` pairs, with adjacent equal-CSS runs joined.
///
/// Compared on what is **emitted** rather than on the `StyleSpan`s — the CSS and
/// the feature list both, since both reach the renderer. Two spans differing
/// only in something this view does not emit are the same span as far as line
/// breaking is concerned, and keeping them apart would be a boundary with no
/// reader.
pub(super) fn coalesce(
    text: &str,
    spans: &[loki_layout::para::StyleSpan],
    families: &FamilyMap,
) -> Vec<Run> {
    let mut out: Vec<Run> = Vec::new();
    for span in spans {
        let css = span_css(span, families);
        let features = super::style::span_font_features(span);
        // `get` rather than indexing: a range past the end would panic inside a
        // render, and a missing run is a visibly shorter paragraph rather than
        // a dead application.
        let slice = text.get(span.range.clone()).unwrap_or_default();
        match out.last_mut() {
            // Both halves of what reaches the renderer, not just the CSS: two
            // runs that agree on style but differ on kerning are two runs.
            Some((prev_css, prev_features, prev_text))
                if *prev_css == css && *prev_features == features =>
            {
                prev_text.push_str(slice)
            }
            _ => out.push((css, features, slice.to_string())),
        }
    }
    out
}

#[cfg(test)]
#[path = "content_tests.rs"]
mod tests;
