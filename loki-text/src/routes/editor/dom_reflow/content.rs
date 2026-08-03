// SPDX-License-Identifier: Apache-2.0

//! `Document` content → RSX, for the DOM reflow view (ADR-0017).
//!
//! # Coverage, stated rather than implied
//!
//! Paragraphs, headings, plain blocks, block quotes, code blocks, line blocks
//! and the inline marks. **Not** tables, lists, images, footnotes, fields,
//! comments or revision marks — those reach the fallback below.
//!
//! An unhandled block renders a visible **placeholder**, not nothing. This view
//! exists to be compared against the canvas path, and a silently dropped table
//! makes the two look closer than they are — the one failure mode a comparison
//! instrument must not have. `TODO(dom-reflow-coverage)`.

use dioxus::prelude::*;
use loki_doc_model::content::block::Block;
use loki_doc_model::content::inline::Inline;
use loki_doc_model::style::catalog::StyleCatalog;

use super::style::{resolved_para_css, span_css};

/// A visible marker for content this view cannot yet render.
///
/// Deliberately loud: see the module docs on why an omission must not look like
/// agreement.
fn unsupported(kind: &str) -> Element {
    rsx! {
        div {
            style: "border: 1px dashed #c0392b; color: #c0392b; padding: 2pt 4pt; \
                    margin: 4pt 0; font-size: 9pt;",
            "[dom-reflow: {kind} not rendered]"
        }
    }
}

/// One inline run.
///
/// Nested marks nest as elements rather than being flattened into one style
/// string: that is how the model states them, and it is what lets an inner run
/// override an outer one the way `char_css`'s explicit-`false` handling expects.
fn inline_el(inline: &Inline) -> Element {
    match inline {
        Inline::Str(s) => rsx! { "{s}" },
        Inline::Space => rsx! { " " },
        Inline::SoftBreak => rsx! { " " },
        Inline::LineBreak => rsx! { br {} },
        Inline::Strong(kids) => rsx! { strong { { inlines(kids) } } },
        Inline::Emph(kids) => rsx! { em { { inlines(kids) } } },
        Inline::Underline(kids) => rsx! {
            span { style: "text-decoration: underline;", { inlines(kids) } }
        },
        Inline::Strikeout(kids) => rsx! {
            span { style: "text-decoration: line-through;", { inlines(kids) } }
        },
        Inline::Superscript(kids) => rsx! { sup { { inlines(kids) } } },
        Inline::Subscript(kids) => rsx! { sub { { inlines(kids) } } },
        Inline::SmallCaps(kids) => rsx! {
            span { style: "font-variant: small-caps;", { inlines(kids) } }
        },
        Inline::Code(_, s) => rsx! { code { "{s}" } },
        Inline::Quoted(_, kids) => rsx! { span { { inlines(kids) } } },
        // A styled run reached through this path has no catalog in scope, so it
        // renders its content and lets the enclosing paragraph's resolved spans
        // carry the formatting. Inside a `StyledPara` — which is every run that
        // comes from a real document — `flatten_paragraph_with_base` has already
        // resolved it, and this arm is never taken.
        Inline::StyledRun(run) => rsx! { span { { inlines(&run.content) } } },
        // Everything else — images, notes, fields, citations, math — is content
        // this view does not carry yet. It renders as a marker rather than as
        // nothing; see the module docs.
        _ => rsx! {
            span {
                style: "color: #c0392b; font-size: 9pt;",
                "[inline]"
            }
        },
    }
}

/// A run of inlines.
pub(super) fn inlines(items: &[Inline]) -> Element {
    rsx! {
        for (i, item) in items.iter().enumerate() {
            { rsx! { span { key: "{i}", { inline_el(item) } } } }
        }
    }
}

/// One block.
pub(super) fn block_el(block: &Block, catalog: &StyleCatalog) -> Element {
    match block {
        Block::Para(items) | Block::Plain(items) => rsx! {
            p { style: "margin: 0 0 6pt 0;", { inlines(items) } }
        },
        Block::StyledPara(p) => styled_para_el(p, catalog),
        Block::Heading(level, _, items) => {
            // One element with a size, rather than `h1`..`h6`: the document's
            // own heading styles decide the size, and borrowing the browser's
            // default scale would make this path disagree with the canvas one
            // for a reason that has nothing to do with the document.
            let size_pt = match level {
                1 => 24.0,
                2 => 18.0,
                3 => 14.0,
                _ => 12.0,
            };
            rsx! {
                p {
                    style: format!(
                        "margin: 12pt 0 6pt 0; font-weight: bold; font-size: {size_pt}pt;"
                    ),
                    { inlines(items) }
                }
            }
        }
        Block::BlockQuote(kids) => rsx! {
            div {
                style: "margin: 6pt 0 6pt 24pt;",
                for (i, b) in kids.iter().enumerate() {
                    { rsx! { div { key: "{i}", { block_el(b, catalog) } } } }
                }
            }
        },
        Block::CodeBlock(_, text) => rsx! {
            pre {
                style: "margin: 6pt 0; font-family: monospace; white-space: pre-wrap;",
                "{text}"
            }
        },
        Block::LineBlock(lines) => rsx! {
            p {
                style: "margin: 0 0 6pt 0;",
                for (i, line) in lines.iter().enumerate() {
                    { rsx! { span { key: "{i}", { inlines(line) } br {} } } }
                }
            }
        },
        Block::HorizontalRule => rsx! {
            div { style: "border-top: 1px solid #888; margin: 8pt 0;" }
        },
        Block::Table(_) => unsupported("table"),
        Block::OrderedList(..) | Block::BulletList(_) => unsupported("list"),
        _ => unsupported("block"),
    }
}

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
fn styled_para_el(
    para: &loki_doc_model::content::block::StyledParagraph,
    catalog: &StyleCatalog,
) -> Element {
    let resolved = loki_layout::resolve::resolve_para_props(para, catalog);
    // The note counter is local and discarded: this view does not render
    // footnotes, and advancing a shared counter for numbers nothing shows would
    // renumber the notes the canvas path does render.
    let mut notes = 0u32;
    let (text, spans, _images, _notes) = loki_layout::resolve::flatten_paragraph_with_base(
        para,
        catalog,
        &mut notes,
        None,
        loki_layout::RevisionDisplay::default(),
    );
    rsx! {
        p {
            style: resolved_para_css(&resolved),
            for (i, span) in spans.iter().enumerate() {
                {
                    // `get` rather than indexing: a range past the end would
                    // panic inside a render, and a missing run is a visibly
                    // shorter paragraph rather than a dead application.
                    let slice = text.get(span.range.clone()).unwrap_or_default().to_string();
                    rsx! { span { key: "{i}", style: span_css(span), "{slice}" } }
                }
            }
        }
    }
}
