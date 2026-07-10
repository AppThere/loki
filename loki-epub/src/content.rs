// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 AppThere Loki contributors

//! Converts the abstract [`loki_doc_model::Document`] content tree into an
//! XHTML body fragment, collecting heading anchors for the navigation document
//! and image resources for packaging.
//!
//! Block-level dispatch lives here; inline rendering is in [`crate::inlines`],
//! table serialisation in [`crate::tables`], and image handling in
//! [`crate::images`].

use loki_doc_model::Document;
use loki_doc_model::content::block::Block;
use loki_doc_model::content::inline::Inline;

use crate::images::EpubImage;
use crate::inlines::{heading_level, plain_text};

/// A single entry in the EPUB table of contents.
pub struct TocEntry {
    /// Heading level (1–6).
    pub level: u8,
    /// Fragment id of the heading element in the content document.
    pub anchor: String,
    /// Plain-text heading label.
    pub text: String,
}

/// The rendered content document, its derived table of contents, and the image
/// resources it references.
pub struct RenderedContent {
    /// The XHTML `<body>` inner markup.
    pub body: String,
    /// Heading anchors in document order.
    pub toc: Vec<TocEntry>,
    /// Packaged image resources referenced by the body.
    pub images: Vec<EpubImage>,
    /// Whether the content embeds MathML (drives the `mathml` manifest property).
    pub has_math: bool,
}

/// Renders all sections of `doc` into a single XHTML body fragment.
#[must_use]
pub fn render_content(doc: &Document) -> RenderedContent {
    let mut ctx = RenderCtx::default();
    let mut body = String::new();
    for section in &doc.sections {
        for block in &section.blocks {
            ctx.render_block(block, &mut body);
        }
    }
    RenderedContent {
        body,
        toc: ctx.toc,
        images: ctx.images,
        has_math: ctx.has_math,
    }
}

/// Shared rendering state. Fields are `pub(crate)` so the per-concern rendering
/// modules ([`crate::inlines`], [`crate::tables`], [`crate::images`]) can
/// contribute `impl RenderCtx` blocks.
#[derive(Default)]
pub(crate) struct RenderCtx {
    pub(crate) toc: Vec<TocEntry>,
    pub(crate) heading_seq: usize,
    pub(crate) images: Vec<EpubImage>,
    pub(crate) image_seq: usize,
    /// Set once any MathML is emitted, so the content document's manifest item
    /// gets `properties="mathml"` (EPUB 3.3 §5.4.2 / 5.4).
    pub(crate) has_math: bool,
}

impl RenderCtx {
    /// Renders a block-level element.
    pub(crate) fn render_block(&mut self, block: &Block, out: &mut String) {
        match block {
            Block::Para(inlines) | Block::Plain(inlines) => {
                out.push_str("<p>");
                self.render_inlines(inlines, out);
                out.push_str("</p>\n");
            }
            Block::StyledPara(sp) => {
                let level = sp.style_id.as_ref().and_then(|s| heading_level(s.as_str()));
                if let Some(lvl) = level {
                    self.render_heading(lvl, &sp.inlines, out);
                } else {
                    out.push_str("<p>");
                    self.render_inlines(&sp.inlines, out);
                    out.push_str("</p>\n");
                }
            }
            Block::Heading(level, _attr, inlines) => {
                self.render_heading((*level).clamp(1, 6), inlines, out);
            }
            Block::BulletList(items) => self.render_list("ul", items, out),
            Block::OrderedList(_attrs, items) => self.render_list("ol", items, out),
            Block::BlockQuote(blocks) => {
                out.push_str("<blockquote>\n");
                for b in blocks {
                    self.render_block(b, out);
                }
                out.push_str("</blockquote>\n");
            }
            Block::CodeBlock(_attr, code) => {
                out.push_str("<pre><code>");
                out.push_str(&crate::xml::escape_text(code));
                out.push_str("</code></pre>\n");
            }
            Block::LineBlock(lines) => {
                out.push_str("<p>");
                for (i, line) in lines.iter().enumerate() {
                    if i > 0 {
                        out.push_str("<br/>");
                    }
                    self.render_inlines(line, out);
                }
                out.push_str("</p>\n");
            }
            Block::HorizontalRule => out.push_str("<hr/>\n"),
            Block::DefinitionList(items) => {
                out.push_str("<dl>\n");
                for (term, defs) in items {
                    out.push_str("<dt>");
                    self.render_inlines(term, out);
                    out.push_str("</dt>\n");
                    for def in defs {
                        out.push_str("<dd>\n");
                        for b in def {
                            self.render_block(b, out);
                        }
                        out.push_str("</dd>\n");
                    }
                }
                out.push_str("</dl>\n");
            }
            Block::Table(table) => self.render_table(table, out),
            Block::Figure(_attr, caption, blocks) => {
                out.push_str("<figure>\n");
                for b in blocks {
                    self.render_block(b, out);
                }
                if !caption.full.is_empty() {
                    out.push_str("<figcaption>\n");
                    for b in &caption.full {
                        self.render_block(b, out);
                    }
                    out.push_str("</figcaption>\n");
                }
                out.push_str("</figure>\n");
            }
            Block::Div(_attr, blocks) => {
                for b in blocks {
                    self.render_block(b, out);
                }
            }
            // Generated/auxiliary blocks carry no reflowable body content.
            Block::RawBlock(_, _)
            | Block::TableOfContents(_)
            | Block::Index(_)
            | Block::NotesBlock(_) => {}
            // `Block` is non-exhaustive; future variants render as nothing.
            _ => {}
        }
    }

    fn render_heading(&mut self, level: u8, inlines: &[Inline], out: &mut String) {
        self.heading_seq += 1;
        let anchor = format!("h{}", self.heading_seq);
        let text = plain_text(inlines);
        out.push_str(&format!("<h{lvl} id=\"{anchor}\">", lvl = level));
        self.render_inlines(inlines, out);
        out.push_str(&format!("</h{lvl}>\n", lvl = level));
        self.toc.push(TocEntry {
            level,
            anchor,
            text,
        });
    }

    fn render_list(&mut self, tag: &str, items: &[Vec<Block>], out: &mut String) {
        out.push_str(&format!("<{tag}>\n"));
        for item in items {
            out.push_str("<li>");
            for b in item {
                self.render_block(b, out);
            }
            out.push_str("</li>\n");
        }
        out.push_str(&format!("</{tag}>\n"));
    }
}

#[cfg(test)]
#[path = "content_tests.rs"]
mod tests;
