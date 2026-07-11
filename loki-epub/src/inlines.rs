// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 AppThere Loki contributors

//! Inline-level XHTML rendering for the EPUB content document.

use loki_doc_model::content::annotation::CommentRefKind;
use loki_doc_model::content::field::{Field, FieldKind};
use loki_doc_model::content::float::{FloatWrap, TextWrap, WrapSide};
use loki_doc_model::content::inline::{Inline, QuoteType};
use loki_doc_model::meta::DocumentMeta;
use loki_doc_model::style::props::char_props::{CharProps, VerticalAlign};

use crate::content::RenderCtx;
use crate::xml::{escape_attr, escape_text};

/// Static values for the metadata-backed field kinds, captured once from
/// [`DocumentMeta`]. An EPUB is reflowable and page-less, so page/date/reference
/// fields have no static value here — they render from their `current_value`
/// snapshot (set at import) or not at all.
#[derive(Default)]
pub(crate) struct FieldEnv {
    title: Option<String>,
    author: Option<String>,
    subject: Option<String>,
}

impl FieldEnv {
    pub(crate) fn from_meta(meta: &DocumentMeta) -> Self {
        Self {
            title: meta.title.clone(),
            author: meta.creator.clone(),
            subject: meta.subject.clone(),
        }
    }
}

/// Maps a floating image's [`FloatWrap`] to an inline CSS `float` declaration so
/// the reflowable EPUB flows text around it (the reflow-target equivalent of the
/// paginated wrap band). Side-wrapping floats only; `TopAndBottom`/behind-text
/// floats stay block-level (`None`). `WrapSide` names the side **text** occupies,
/// so the float sits opposite (mirrors `loki-layout`'s `plan_float`).
pub(crate) fn float_css(wrap: &FloatWrap) -> Option<&'static str> {
    if wrap.behind_text || matches!(wrap.wrap, TextWrap::TopAndBottom) {
        return None;
    }
    Some(match wrap.side {
        // Text on the left → image floats right; otherwise image floats left.
        WrapSide::Left => "float:right; margin:0 0 0.4em 0.8em; max-width:45%; height:auto;",
        _ => "float:left; margin:0 0.8em 0.4em 0; max-width:45%; height:auto;",
    })
}

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
            Inline::Image(attr, alt, target) => {
                let alt_text = plain_text(alt);
                // A floating image becomes a CSS `float` so text wraps around it.
                let style = FloatWrap::read(attr).as_ref().and_then(float_css);
                self.render_image(&target.url, &alt_text, style, out);
            }
            Inline::StyledRun(run) => {
                self.render_styled_run(run.direct_props.as_deref(), &run.content, out)
            }
            Inline::Span(_, c) | Inline::Cite(_, c) => self.render_inlines(c, out),
            Inline::Math(_math_type, mathml) => {
                // EPUB 3.3 renders MathML natively (§5.4). The model stores a
                // complete, namespaced `<math>` element, so emit it verbatim
                // (not escaped — it is markup) and flag the content document so
                // its manifest item declares `properties="mathml"`.
                out.push_str(mathml);
                self.has_math = true;
            }
            Inline::Field(f) => {
                // Fields resolve to static text in a reflowable EPUB (no live
                // page/date context): the `current_value` snapshot, else a
                // metadata-backed value. Unresolvable fields render as nothing.
                if let Some(text) = self.field_text(f) {
                    out.push_str("<span class=\"field\">");
                    out.push_str(&escape_text(&text));
                    out.push_str("</span>");
                }
            }
            Inline::Comment(c) => self.render_comment_ref(c, out),
            Inline::Note(_, _) | Inline::RawInline(_, _) | Inline::Bookmark(_, _) => {}
            // `Inline` is non-exhaustive; future variants render as nothing.
            _ => {}
        }
    }

    /// Resolves a field to its display text, or `None` when it has no static
    /// value in a page-less EPUB. Prefers the `current_value` snapshot, then the
    /// metadata-backed value for Title/Author/Subject.
    fn field_text(&self, f: &Field) -> Option<String> {
        if let Some(v) = &f.current_value
            && !v.is_empty()
        {
            return Some(v.clone());
        }
        match &f.kind {
            FieldKind::Title => self.field_env.title.clone(),
            FieldKind::Author => self.field_env.author.clone(),
            FieldKind::Subject => self.field_env.subject.clone(),
            _ => None,
        }
    }

    /// Emits an inline reference marker for a comment anchor. Only the start (or
    /// point) anchor produces a marker — a superscript link to the trailing
    /// `<aside>` (`content.rs::render_comment_asides`); the end anchor and
    /// unknown ids render nothing. The comment is registered in `comment_seq` on
    /// first reference so its number and aside are produced.
    fn render_comment_ref(
        &mut self,
        c: &loki_doc_model::content::annotation::CommentRef,
        out: &mut String,
    ) {
        if matches!(c.kind, CommentRefKind::End) || !self.comments.contains_key(&c.id) {
            return;
        }
        let number = match self.comment_seq.iter().position(|id| id == &c.id) {
            Some(idx) => idx + 1,
            None => {
                self.comment_seq.push(c.id.clone());
                self.comment_seq.len()
            }
        };
        out.push_str(&format!(
            "<sup class=\"comment-ref\"><a href=\"#cmt-{id}\" epub:type=\"noteref\" \
             role=\"doc-noteref\">{number}</a></sup>",
            id = escape_attr(&c.id)
        ));
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
