// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 AppThere Loki contributors

//! CPU-side flat document renderer for Android (no `android_gpu`).
//!
//! Renders the document as a continuous scrollable flow of HTML elements so the
//! CPU Vello renderer can display content without GPU compute shaders.  A flat
//! web-style layout is used instead of paginated layout to avoid horizontal
//! overflow on narrow mobile screens and to reduce visible fidelity loss.
//!
//! Formatting fidelity is intentionally limited — inline bold/italic/strikeout
//! are preserved; exact glyph positions, images, and tables are not.

use std::sync::Arc;

use dioxus::prelude::*;
use loki_doc_model::content::block::Block;
use loki_doc_model::content::inline::Inline;

use crate::doc_page_source::DocPageSource;

// ── Text extraction ───────────────────────────────────────────────────────────

fn html_escape(s: &str) -> String {
    s.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
}

fn inline_html(inline: &Inline) -> String {
    match inline {
        Inline::Str(s) => html_escape(s),
        Inline::Space | Inline::SoftBreak | Inline::LineBreak => " ".to_string(),
        Inline::Emph(children) => format!("<em>{}</em>", inlines_html(children)),
        Inline::Strong(children) => format!("<strong>{}</strong>", inlines_html(children)),
        Inline::Strikeout(children) => format!("<s>{}</s>", inlines_html(children)),
        Inline::Code(_, s) => format!("<code>{}</code>", html_escape(s)),
        Inline::StyledRun(run) => {
            let mut out = inlines_html(&run.content);
            if run.direct_props.as_ref().map(|p| p.strikethrough.is_some()).unwrap_or(false) {
                out = format!("<s>{out}</s>");
            }
            if run.direct_props.as_ref().and_then(|p| p.italic).unwrap_or(false) {
                out = format!("<em>{out}</em>");
            }
            if run.direct_props.as_ref().and_then(|p| p.bold).unwrap_or(false) {
                out = format!("<strong>{out}</strong>");
            }
            out
        }
        _ => String::new(),
    }
}

fn inlines_html(inlines: &[Inline]) -> String {
    inlines.iter().map(inline_html).collect()
}

/// Convert a block to an `(inner_html, css_style)` pair, or `None` to skip.
fn block_content(block: &Block) -> Option<(String, String)> {
    const BODY: &str = "margin: 0 0 10px 0; font-size: 16px; line-height: 1.6;";
    const BLANK: &str = "margin: 0 0 6px 0;";

    match block {
        Block::Para(inlines) | Block::Plain(inlines) => {
            let html = inlines_html(inlines);
            if html.trim().is_empty() {
                return Some(("&nbsp;".to_string(), BLANK.to_string()));
            }
            Some((html, BODY.to_string()))
        }
        // Primary paragraph type produced by OOXML/ODF import.
        Block::StyledPara(sp) => {
            let html = inlines_html(&sp.inlines);
            if html.trim().is_empty() {
                return Some(("&nbsp;".to_string(), BLANK.to_string()));
            }
            Some((html, BODY.to_string()))
        }
        Block::Heading(level, _, inlines) => {
            let html = inlines_html(inlines);
            if html.trim().is_empty() {
                return None;
            }
            let (size, margin_top) = match level {
                1 => (28u32, 20u32),
                2 => (24, 16),
                3 => (20, 12),
                _ => (18, 10),
            };
            Some((
                html,
                format!(
                    "margin: {margin_top}px 0 8px 0; font-size: {size}px; \
                     font-weight: bold; line-height: 1.3;"
                ),
            ))
        }
        Block::BlockQuote(inner_blocks) => {
            let text: String = inner_blocks
                .iter()
                .flat_map(|b| block_content(b))
                .map(|(h, _)| h)
                .collect::<Vec<_>>()
                .join(" ");
            if text.trim().is_empty() {
                return None;
            }
            Some((
                text,
                "margin: 0 0 10px 0; padding-left: 14px; \
                 border-left: 3px solid #ccc; font-size: 16px; \
                 color: #555; font-style: italic; line-height: 1.6;"
                    .to_string(),
            ))
        }
        _ => None,
    }
}

// ── CpuDocView ────────────────────────────────────────────────────────────────

/// Props for [`CpuDocView`].
#[derive(Clone, Props)]
pub(crate) struct CpuDocViewProps {
    pub(crate) source: Arc<DocPageSource>,
    /// Incremented on every document mutation so the component re-renders.
    pub(crate) doc_gen: u64,
}

impl PartialEq for CpuDocViewProps {
    fn eq(&self, other: &Self) -> bool {
        Arc::ptr_eq(&self.source, &other.source) && self.doc_gen == other.doc_gen
    }
}

/// Flat web-style document renderer for the Android CPU path.
///
/// Renders all blocks in document order as a single fluid column.  Uses the
/// device's default `sans-serif` font and adapts to the viewport width so that
/// text does not overflow horizontally on narrow screens.
#[allow(non_snake_case)]
pub(crate) fn CpuDocView(props: CpuDocViewProps) -> Element {
    let doc = props.source.document();
    let items: Vec<(String, String)> = doc.blocks_flat().filter_map(block_content).collect();

    rsx! {
        div {
            // Prefer bundled metric-compatible families (injected via @font-face
            // in loki-text/src/app.rs) then fall back to system generic families.
            // Explicit white background so the gray canvas behind DocumentView
            // does not bleed through the reflowable content area.
            style: "font-family: 'Carlito', 'Arimo', sans-serif; padding: 16px; \
                    box-sizing: border-box; color: #1a1a1a; width: 100%; \
                    background: #FAFAFA;",
            for (inner_html, div_style) in &items {
                div {
                    style: "{div_style}",
                    dangerous_inner_html: "{inner_html}",
                }
            }
        }
    }
}
