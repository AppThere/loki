// SPDX-License-Identifier: Apache-2.0

//! `Document` content → RSX, for the DOM reflow view (ADR-0017).
//!
//! # Coverage, stated rather than implied
//!
//! Paragraphs, headings, plain blocks, block quotes, code blocks, line blocks,
//! the inline marks, **tables** (`super::table`) and a paragraph's **images**
//! (`super::image`). **Not** lists, footnotes, fields, comments or revision
//! marks — those reach the fallback below.
//!
//! An unhandled block renders a visible **placeholder**, not nothing. This view
//! exists to be compared against the canvas path, and a silently dropped table
//! makes the two look closer than they are — the one failure mode a comparison
//! instrument must not have. `TODO(dom-reflow-coverage)`.

use dioxus::prelude::*;
use loki_doc_model::content::block::Block;
use loki_doc_model::content::inline::Inline;
use std::collections::BTreeMap;

use loki_doc_model::style::catalog::StyleCatalog;

use super::content_para::styled_para_el;

/// Requested family name → the family that will actually be used.
///
/// Built once per render by [`super::resolve_families`] from
/// `FontResources::resolve_font_name`, which is the substitution policy the
/// canvas path applies. Emitting the *requested* family instead lets Blitz fall
/// back its own way, so a document with a missing font sets differently on the
/// two paths — measured on a screenplay, where one path was monospaced and the
/// other proportional.
pub type FamilyMap = BTreeMap<String, String>;

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
pub(super) fn block_el(block: &Block, catalog: &StyleCatalog, families: &FamilyMap) -> Element {
    match block {
        // Synthesised into the styled paragraph the resolver understands, then
        // resolved — the same two steps the canvas path takes for these blocks.
        // Rendering them from their raw inlines instead is what left the heading
        // proportional while the body came out monospaced.
        Block::Para(items) | Block::Plain(items) => styled_para_el(
            &loki_layout::flow::synthesize_plain_para(items),
            catalog,
            families,
        ),
        Block::StyledPara(p) => styled_para_el(p, catalog, families),
        Block::Heading(level, attr, items) => styled_para_el(
            &loki_layout::flow::synthesize_heading_para(*level, attr, items),
            catalog,
            families,
        ),
        Block::BlockQuote(kids) => rsx! {
            div {
                style: "margin: 6pt 0 6pt 24pt;",
                for (i, b) in kids.iter().enumerate() {
                    { rsx! { div { key: "{i}", { block_el(b, catalog, families) } } } }
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
        // T7.3: a table cannot be scaled to the column, so it goes in its own
        // scrollport rather than widening the page. See `oversized`.
        Block::Table(t) => rsx! {
            super::oversized::AtOversized {
                fittable: false,
                { super::table::table_el(t, catalog, families) }
            }
        },
        Block::OrderedList(..) | Block::BulletList(_) => unsupported("list"),
        _ => unsupported("block"),
    }
}

/// Every font family the document's runs ask for.
///
/// Uses the same flattener the renderer does, so the set is exactly the families
/// that will be emitted — a collector that walked the catalog instead could miss
/// one a direct run introduced, and a family that missed the map would be the
/// only one still emitted unsubstituted.
pub(super) fn requested_families(doc: &loki_doc_model::document::Document) -> Vec<String> {
    let mut out: Vec<String> = Vec::new();
    for section in &doc.sections {
        for block in &section.blocks {
            // The same synthesis the renderer applies, so a heading's family
            // reaches the map. Collecting only `StyledPara` would leave every
            // heading emitting its requested family unsubstituted — the one run
            // still rendered with a policy that is not ours.
            let owned;
            let para = match block {
                Block::StyledPara(p) => p,
                Block::Para(items) | Block::Plain(items) => {
                    owned = loki_layout::flow::synthesize_plain_para(items);
                    &owned
                }
                Block::Heading(level, attr, items) => {
                    owned = loki_layout::flow::synthesize_heading_para(*level, attr, items);
                    &owned
                }
                _ => continue,
            };
            let mut notes = 0u32;
            let (_, spans, _, _) = loki_layout::resolve::flatten_paragraph_with_base(
                para,
                &doc.styles,
                &mut notes,
                None,
                loki_layout::RevisionDisplay::default(),
            );
            for span in &spans {
                if let Some(name) = &span.font_name
                    && !out.iter().any(|n| n == name)
                {
                    out.push(name.clone());
                }
            }
        }
    }
    out
}
