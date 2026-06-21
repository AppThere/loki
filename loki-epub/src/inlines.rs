// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 AppThere Loki contributors

//! Inline-level XHTML rendering for the EPUB content document.

use loki_doc_model::content::inline::{Inline, QuoteType};
use loki_doc_model::style::props::char_props::{CharProps, VerticalAlign};

use crate::content::RenderCtx;
use crate::xml::{escape_attr, escape_text};

impl RenderCtx {
    /// Renders a sequence of inlines.
    pub(crate) fn render_inlines(&mut self, inlines: &[Inline], out: &mut String) {
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
            Inline::Image(_attr, alt, target) => {
                let alt_text = plain_text(alt);
                self.render_image(&target.url, &alt_text, out);
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
pub(crate) fn heading_level(style_id: &str) -> Option<u8> {
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

/// Extracts the concatenated plain text of an inline sequence (for TOC labels
/// and image alt text).
pub(crate) fn plain_text(inlines: &[Inline]) -> String {
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
