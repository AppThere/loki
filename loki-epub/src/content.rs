// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 AppThere Loki contributors

//! Converts the abstract [`loki_doc_model::Document`] content tree into an
//! XHTML body fragment, collecting heading anchors for the navigation document.

use loki_doc_model::Document;
use loki_doc_model::content::block::Block;
use loki_doc_model::content::inline::{Inline, QuoteType};
use loki_doc_model::style::props::char_props::{CharProps, VerticalAlign};

use crate::xml::{escape_attr, escape_text};

/// A single entry in the EPUB table of contents.
pub struct TocEntry {
    /// Heading level (1–6).
    pub level: u8,
    /// Fragment id of the heading element in the content document.
    pub anchor: String,
    /// Plain-text heading label.
    pub text: String,
}

/// The rendered content document and its derived table of contents.
pub struct RenderedContent {
    /// The XHTML `<body>` inner markup.
    pub body: String,
    /// Heading anchors in document order.
    pub toc: Vec<TocEntry>,
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
    RenderedContent { body, toc: ctx.toc }
}

#[derive(Default)]
struct RenderCtx {
    toc: Vec<TocEntry>,
    heading_seq: usize,
}

impl RenderCtx {
    fn render_block(&mut self, block: &Block, out: &mut String) {
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
                out.push_str(&escape_text(code));
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
            Block::Table(_) => {
                // TODO(epub-table): full table serialisation is deferred; emit
                // a placeholder paragraph so content order is preserved.
                out.push_str("<p class=\"loki-table-placeholder\">[table]</p>\n");
            }
            Block::Figure(_, _, blocks) | Block::Div(_, blocks) => {
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

    fn render_inlines(&mut self, inlines: &[Inline], out: &mut String) {
        for inline in inlines {
            self.render_inline(inline, out);
        }
    }

    fn render_inline(&mut self, inline: &Inline, out: &mut String) {
        match inline {
            Inline::Str(s) => out.push_str(&escape_text(s)),
            Inline::Space | Inline::SoftBreak => out.push(' '),
            Inline::LineBreak => out.push_str("<br/>"),
            Inline::Emph(c) => self.wrap("em", c, out),
            Inline::Strong(c) => self.wrap("strong", c, out),
            Inline::Underline(c) => self.wrap_span("text-decoration:underline", c, out),
            Inline::Strikeout(c) => self.wrap("s", c, out),
            Inline::Superscript(c) => self.wrap("sup", c, out),
            Inline::Subscript(c) => self.wrap("sub", c, out),
            Inline::SmallCaps(c) => self.wrap_span("font-variant:small-caps", c, out),
            Inline::Code(_, s) => {
                out.push_str("<code>");
                out.push_str(&escape_text(s));
                out.push_str("</code>");
            }
            Inline::Quoted(kind, c) => {
                let (open, close) = match kind {
                    QuoteType::SingleQuote => ('\u{2018}', '\u{2019}'),
                    QuoteType::DoubleQuote => ('\u{201C}', '\u{201D}'),
                };
                out.push(open);
                self.render_inlines(c, out);
                out.push(close);
            }
            Inline::Link(_attr, c, target) => {
                out.push_str(&format!("<a href=\"{}\">", escape_attr(&target.url)));
                self.render_inlines(c, out);
                out.push_str("</a>");
            }
            Inline::Image(_attr, alt, _target) => {
                // TODO(epub-image): image resources are not yet packaged; emit
                // the alt text so meaning is preserved.
                self.render_inlines(alt, out);
            }
            Inline::StyledRun(run) => {
                self.render_styled_run(run.direct_props.as_deref(), &run.content, out)
            }
            Inline::Span(_, c) | Inline::Cite(_, c) => self.render_inlines(c, out),
            Inline::Note(_, _)
            | Inline::Math(_, _)
            | Inline::RawInline(_, _)
            | Inline::Field(_)
            | Inline::Comment(_)
            | Inline::Bookmark(_, _) => {}
            // `Inline` is non-exhaustive; future variants render as nothing.
            _ => {}
        }
    }

    fn render_styled_run(
        &mut self,
        props: Option<&CharProps>,
        content: &[Inline],
        out: &mut String,
    ) {
        let mut close: Vec<&str> = Vec::new();
        if let Some(p) = props {
            if p.bold == Some(true) {
                out.push_str("<strong>");
                close.push("</strong>");
            }
            if p.italic == Some(true) {
                out.push_str("<em>");
                close.push("</em>");
            }
            if p.strikethrough.is_some() {
                out.push_str("<s>");
                close.push("</s>");
            }
            match p.vertical_align {
                Some(VerticalAlign::Superscript) => {
                    out.push_str("<sup>");
                    close.push("</sup>");
                }
                Some(VerticalAlign::Subscript) => {
                    out.push_str("<sub>");
                    close.push("</sub>");
                }
                _ => {}
            }
            if p.underline.is_some() {
                out.push_str("<span style=\"text-decoration:underline\">");
                close.push("</span>");
            }
        }
        self.render_inlines(content, out);
        for tag in close.iter().rev() {
            out.push_str(tag);
        }
    }

    fn wrap(&mut self, tag: &str, content: &[Inline], out: &mut String) {
        out.push_str(&format!("<{tag}>"));
        self.render_inlines(content, out);
        out.push_str(&format!("</{tag}>"));
    }

    fn wrap_span(&mut self, style: &str, content: &[Inline], out: &mut String) {
        out.push_str(&format!("<span style=\"{style}\">"));
        self.render_inlines(content, out);
        out.push_str("</span>");
    }
}

/// Maps a paragraph style id to a heading level, if it names one.
///
/// Recognises `Heading1`…`Heading6`, `Heading 1`…`Heading 6`, and the bare
/// `Title` style (level 1).
fn heading_level(style_id: &str) -> Option<u8> {
    let lower = style_id.to_ascii_lowercase();
    if lower == "title" {
        return Some(1);
    }
    let digits: String = lower.trim_start_matches("heading").trim().to_string();
    if lower.starts_with("heading")
        && let Ok(n) = digits.parse::<u8>()
    {
        return Some(n.clamp(1, 6));
    }
    None
}

/// Extracts the concatenated plain text of an inline sequence (for TOC labels).
fn plain_text(inlines: &[Inline]) -> String {
    let mut s = String::new();
    collect_plain(inlines, &mut s);
    s.trim().to_string()
}

fn collect_plain(inlines: &[Inline], out: &mut String) {
    for inline in inlines {
        match inline {
            Inline::Str(t) => out.push_str(t),
            Inline::Space | Inline::SoftBreak | Inline::LineBreak => out.push(' '),
            Inline::Code(_, t) => out.push_str(t),
            Inline::Emph(c)
            | Inline::Strong(c)
            | Inline::Underline(c)
            | Inline::Strikeout(c)
            | Inline::Superscript(c)
            | Inline::Subscript(c)
            | Inline::SmallCaps(c)
            | Inline::Quoted(_, c)
            | Inline::Span(_, c)
            | Inline::Link(_, c, _)
            | Inline::Image(_, c, _)
            | Inline::Cite(_, c) => collect_plain(c, out),
            Inline::StyledRun(run) => collect_plain(&run.content, out),
            _ => {}
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use loki_doc_model::content::attr::NodeAttr;

    #[test]
    fn paragraph_and_heading() {
        let mut doc = Document::new();
        let sec = doc.first_section_mut().unwrap();
        sec.blocks.clear();
        sec.blocks.push(Block::Heading(
            1,
            NodeAttr::default(),
            vec![Inline::Str("Title".into())],
        ));
        sec.blocks
            .push(Block::Para(vec![Inline::Str("Body".into())]));
        let rendered = render_content(&doc);
        assert!(rendered.body.contains("<h1 id=\"h1\">Title</h1>"));
        assert!(rendered.body.contains("<p>Body</p>"));
        assert_eq!(rendered.toc.len(), 1);
        assert_eq!(rendered.toc[0].text, "Title");
    }

    #[test]
    fn escapes_special_characters() {
        let mut doc = Document::new();
        let sec = doc.first_section_mut().unwrap();
        sec.blocks.clear();
        sec.blocks
            .push(Block::Para(vec![Inline::Str("a < b & c".into())]));
        let rendered = render_content(&doc);
        assert!(rendered.body.contains("a &lt; b &amp; c"));
    }
}
