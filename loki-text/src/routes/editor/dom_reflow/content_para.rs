// SPDX-License-Identifier: Apache-2.0

//! A styled paragraph and its runs, for the DOM reflow view.
//!
//! Split from [`super::content`] for the 300-line ceiling: the block dispatch
//! and the inline marks stay there, and the half that goes through
//! `loki_layout`'s resolver lives here.

use dioxus::prelude::*;
use loki_doc_model::style::catalog::StyleCatalog;

use super::content::FamilyMap;
use super::style::{resolved_para_css, span_css};

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
    let resolved = loki_layout::resolve::resolve_para_props(para, catalog);
    // The note counter is local and discarded: this view does not render
    // footnotes, and advancing a shared counter for numbers nothing shows would
    // renumber the notes the canvas path does render.
    let mut notes = 0u32;
    let (text, spans, images, _notes) = loki_layout::resolve::flatten_paragraph_with_base(
        para,
        catalog,
        &mut notes,
        None,
        loki_layout::RevisionDisplay::default(),
    );
    rsx! {
        // Images first, as a block-level prefix — the same placement
        // `stack_block_images` gives them on the canvas path, where Parley has no
        // inline image box either (`TODO(inline-image-flow)`).
        { super::image::images_el(&images) }
        p {
            style: resolved_para_css(&resolved),
            for (i, (css, features, slice)) in
                coalesce(&text, &spans, families).into_iter().enumerate()
            {
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
) -> Vec<(String, &'static str, String)> {
    let mut out: Vec<(String, &'static str, String)> = Vec::new();
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
