// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 AppThere Loki contributors

//! Inline-run serialisation for `content.xml` (text spans, links, footnotes,
//! bookmarks, fields, and embedded images).

use loki_doc_model::content::annotation::{CommentRef, CommentRefKind};
use loki_doc_model::content::block::Block;
use loki_doc_model::content::field::types::{CrossRefFormat, Field, FieldKind};
use loki_doc_model::content::inline::{BookmarkKind, Inline, NoteKind};
use loki_doc_model::style::props::char_props::{
    CharProps, StrikethroughStyle, UnderlineStyle, VerticalAlign,
};

use super::content::{Cx, write_block};
use super::xml::{attr, escape};

/// Writes a sequence of inline runs.
pub(super) fn write_inlines(out: &mut String, inlines: &[Inline], cx: &mut Cx) {
    for inl in inlines {
        write_inline(out, inl, cx);
    }
}

/// Writes one inline run.
fn write_inline(out: &mut String, inl: &Inline, cx: &mut Cx) {
    match inl {
        Inline::Str(s) | Inline::Code(_, s) => out.push_str(&escape(s)),
        Inline::Space | Inline::SoftBreak => out.push(' '),
        Inline::LineBreak => out.push_str("<text:line-break/>"),
        Inline::Strong(c) => span(out, c, cx, set_bold),
        Inline::Emph(c) => span(out, c, cx, set_italic),
        Inline::Underline(c) => span(out, c, cx, set_underline),
        Inline::Strikeout(c) => span(out, c, cx, set_strikethrough),
        Inline::Superscript(c) => span(out, c, cx, set_superscript),
        Inline::Subscript(c) => span(out, c, cx, set_subscript),
        Inline::SmallCaps(c) => span(out, c, cx, set_small_caps),
        Inline::Span(_, c) | Inline::Quoted(_, c) | Inline::Cite(_, c) => write_inlines(out, c, cx),
        Inline::StyledRun(sr) => super::revisions::write_styled_run(out, sr, cx),
        Inline::Link(_, c, target) => {
            out.push_str("<text:a");
            attr(out, "xlink:href", &target.url);
            out.push('>');
            write_inlines(out, c, cx);
            out.push_str("</text:a>");
        }
        Inline::Bookmark(kind, name) => {
            let tag = if matches!(kind, BookmarkKind::End) {
                "bookmark-end"
            } else {
                "bookmark-start"
            };
            out.push_str(&format!("<text:{tag}"));
            attr(out, "text:name", name);
            out.push_str("/>");
        }
        Inline::Field(field) => write_field(out, field),
        Inline::Image(_, alt, target) => write_image(out, alt, &target.url, cx),
        Inline::Note(kind, blocks) => note(out, *kind, blocks, cx),
        Inline::Comment(c) => write_comment(out, c, cx),
        Inline::Math(_, mathml) => write_math(out, mathml, cx),
        // RawInline: no faithful ODF inline representation.
        _ => {}
    }
}

/// Writes a `<draw:frame><draw:object/></draw:frame>` referencing an embedded
/// formula object, registering its `MathML` `content.xml` with the collector.
/// ODF stores math as a sub-document; see [`crate::odt::math`].
fn write_math(out: &mut String, mathml: &str, cx: &mut Cx) {
    let dir = format!("Object {}", cx.objects.len() + 1);
    out.push_str("<draw:frame");
    attr(out, "text:anchor-type", "as-char");
    attr(out, "draw:name", &dir);
    out.push_str("><draw:object");
    attr(out, "xlink:href", &format!("./{dir}"));
    attr(out, "xlink:type", "simple");
    attr(out, "xlink:show", "embed");
    attr(out, "xlink:actuate", "onLoad");
    out.push_str("/></draw:frame>");
    cx.objects.push(super::media::MathPart {
        dir,
        content_xml: crate::odt::math::object_content_xml(mathml),
    });
}

/// Writes a comment anchor: `office:annotation` (with body) at the start/point,
/// `office:annotation-end` at the end. ODF 1.3 §14.1.
fn write_comment(out: &mut String, c: &CommentRef, cx: &Cx) {
    if matches!(c.kind, CommentRefKind::End) {
        out.push_str("<office:annotation-end");
        attr(out, "office:name", &c.id);
        out.push_str("/>");
        return;
    }
    out.push_str("<office:annotation");
    attr(out, "office:name", &c.id);
    out.push('>');
    if let Some(comment) = cx.comments.get(&c.id) {
        if let Some(author) = &comment.author {
            out.push_str("<dc:creator>");
            out.push_str(&escape(author));
            out.push_str("</dc:creator>");
        }
        if let Some(date) = &comment.date {
            out.push_str("<dc:date>");
            out.push_str(&escape(
                &date.to_rfc3339_opts(chrono::SecondsFormat::Secs, false),
            ));
            out.push_str("</dc:date>");
        }
        for block in &comment.body {
            out.push_str("<text:p>");
            out.push_str(&escape(&block_plain_text(block)));
            out.push_str("</text:p>");
        }
    }
    out.push_str("</office:annotation>");
}

/// Concatenates the plain text of a paragraph-like [`Block`].
fn block_plain_text(block: &Block) -> String {
    let inlines = match block {
        Block::Para(i) | Block::Plain(i) => i.as_slice(),
        Block::StyledPara(sp) => sp.inlines.as_slice(),
        _ => &[],
    };
    inlines
        .iter()
        .map(|i| match i {
            Inline::Str(s) => s.as_str(),
            Inline::Space => " ",
            _ => "",
        })
        .collect()
}

/// Writes an ODF field element for `field`.
fn write_field(out: &mut String, field: &Field) {
    match &field.kind {
        FieldKind::PageNumber => {
            out.push_str("<text:page-number text:select-page=\"current\">1</text:page-number>");
        }
        FieldKind::PageCount => out.push_str("<text:page-count>1</text:page-count>"),
        FieldKind::Date { .. } => out.push_str("<text:date>0000-00-00</text:date>"),
        FieldKind::Time { .. } => out.push_str("<text:time>00:00:00</text:time>"),
        FieldKind::Title => out.push_str("<text:title/>"),
        FieldKind::Subject => out.push_str("<text:subject/>"),
        FieldKind::Author => out.push_str("<text:author-name/>"),
        FieldKind::FileName => out.push_str("<text:file-name/>"),
        FieldKind::WordCount => out.push_str("<text:word-count>0</text:word-count>"),
        FieldKind::CrossReference { target, format } => {
            let fmt = match format {
                CrossRefFormat::Number => "number",
                CrossRefFormat::Page => "page",
                CrossRefFormat::Caption => "caption",
                _ => "text",
            };
            out.push_str("<text:bookmark-ref");
            attr(out, "text:reference-format", fmt);
            attr(out, "text:ref-name", target);
            out.push_str("/>");
        }
        FieldKind::Raw { .. } | _ => {}
    }
}

/// Writes a `<draw:frame><draw:image/></draw:frame>` for an embedded or linked
/// image, registering the bytes with the media collector.
fn write_image(out: &mut String, alt: &[Inline], url: &str, cx: &mut Cx) {
    let Some(href) = cx.media.add_image(url) else {
        return;
    };
    out.push_str("<draw:frame");
    attr(out, "text:anchor-type", "as-char");
    attr(out, "draw:name", &href);
    // NB: do not emit `xlink:type` — the importer reads any `type` local-name
    // as the image media type, and the real type is inferred from the part's
    // extension.
    out.push_str("><draw:image");
    attr(out, "xlink:href", &href);
    attr(out, "xlink:show", "embed");
    attr(out, "xlink:actuate", "onLoad");
    out.push('>');
    let alt_text = plain_text(alt);
    if !alt_text.is_empty() {
        out.push_str(&format!("<svg:title>{}</svg:title>", escape(&alt_text)));
    }
    out.push_str("</draw:image></draw:frame>");
}

/// Flattens inline runs to their plain text (image alt text, tracked-deletion
/// content). Recurses through styled runs so a formatted deletion keeps its text.
pub(super) fn plain_text(inlines: &[Inline]) -> String {
    let mut s = String::new();
    for inl in inlines {
        match inl {
            Inline::Str(t) | Inline::Code(_, t) => s.push_str(t),
            Inline::Space | Inline::SoftBreak | Inline::LineBreak => s.push(' '),
            Inline::Strong(c) | Inline::Emph(c) | Inline::Underline(c) | Inline::Span(_, c) => {
                s.push_str(&plain_text(c));
            }
            Inline::StyledRun(sr) => s.push_str(&plain_text(&sr.content)),
            _ => {}
        }
    }
    s
}

fn set_bold(p: &mut CharProps) {
    p.bold = Some(true);
}
fn set_italic(p: &mut CharProps) {
    p.italic = Some(true);
}
fn set_underline(p: &mut CharProps) {
    p.underline = Some(UnderlineStyle::Single);
}
fn set_strikethrough(p: &mut CharProps) {
    p.strikethrough = Some(StrikethroughStyle::Single);
}
fn set_superscript(p: &mut CharProps) {
    p.vertical_align = Some(VerticalAlign::Superscript);
}
fn set_subscript(p: &mut CharProps) {
    p.vertical_align = Some(VerticalAlign::Subscript);
}
fn set_small_caps(p: &mut CharProps) {
    p.small_caps = Some(true);
}

/// Wraps `children` in a `<text:span>` whose automatic text style is built by
/// applying `f` to a default [`CharProps`].
fn span(out: &mut String, children: &[Inline], cx: &mut Cx, f: impl FnOnce(&mut CharProps)) {
    let mut cp = CharProps::default();
    f(&mut cp);
    let name = cx.auto.text_style(&cp);
    wrap_span(out, name.as_deref(), children, cx);
}

/// Wraps `children` in a `<text:span text:style-name=...>`; emits them bare when
/// `name` is `None`.
pub(super) fn wrap_span(out: &mut String, name: Option<&str>, children: &[Inline], cx: &mut Cx) {
    if let Some(name) = name {
        out.push_str("<text:span");
        attr(out, "text:style-name", name);
        out.push('>');
        write_inlines(out, children, cx);
        out.push_str("</text:span>");
    } else {
        write_inlines(out, children, cx);
    }
}

/// Writes a footnote / endnote.
fn note(
    out: &mut String,
    kind: NoteKind,
    blocks: &[loki_doc_model::content::block::Block],
    cx: &mut Cx,
) {
    let class = match kind {
        NoteKind::Endnote => "endnote",
        _ => "footnote",
    };
    out.push_str(&format!(
        "<text:note text:note-class=\"{class}\"><text:note-citation></text:note-citation>\
         <text:note-body>"
    ));
    for b in blocks {
        write_block(out, b, cx);
    }
    out.push_str("</text:note-body></text:note>");
}
