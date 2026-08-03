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

use super::style::{char_css, para_css};

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
        // A styled run: its direct properties become the span's style. The
        // named `style_id` is not resolved — see the `StyledPara` note below,
        // which is the same gap for the same reason.
        Inline::StyledRun(run) => rsx! {
            span {
                style: run.direct_props.as_deref().map(char_css).unwrap_or_default(),
                { inlines(&run.content) }
            }
        },
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
pub(super) fn block_el(block: &Block) -> Element {
    match block {
        Block::Para(items) | Block::Plain(items) => rsx! {
            p { style: "margin: 0 0 6pt 0;", { inlines(items) } }
        },
        Block::StyledPara(p) => {
            // Direct properties only. A style *reference* resolves through the
            // catalog, which this view does not consult — so a document whose
            // formatting lives in named styles renders unstyled here, and that
            // is the largest single gap between the two paths today.
            // TODO(dom-reflow-styles): resolve through `StyleCatalog`.
            let css = format!(
                "margin: 0 0 6pt 0; {}{}",
                p.direct_para_props
                    .as_deref()
                    .map(para_css)
                    .unwrap_or_default(),
                p.direct_char_props
                    .as_deref()
                    .map(char_css)
                    .unwrap_or_default(),
            );
            rsx! { p { style: css, { inlines(&p.inlines) } } }
        }
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
                    { rsx! { div { key: "{i}", { block_el(b) } } } }
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
