// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 AppThere Loki contributors

//! Comment serialization: the in-flow range anchors (`w:commentRangeStart` /
//! `w:commentRangeEnd` / `w:commentReference`) and the `word/comments.xml` part.
//! ECMA-376 §17.13.4.

use quick_xml::Writer;

use loki_doc_model::content::annotation::{Comment, CommentRef, CommentRefKind};
use loki_doc_model::content::block::Block;
use loki_doc_model::content::inline::Inline;

use super::xml::{NS_W, write_empty, write_end, write_start, wval};

/// Writes the in-flow anchor for a [`CommentRef`].
///
/// `Start`/`End` emit the matching range markers; `End` and `Point` also emit a
/// `w:commentReference` run (styled `CommentReference`) so Word renders the
/// comment marker.
pub(super) fn write_comment_ref<W: std::io::Write>(w: &mut Writer<W>, c: &CommentRef) {
    match c.kind {
        CommentRefKind::Start => {
            let _ = write_empty(w, "w:commentRangeStart", &[("w:id", &c.id)]);
        }
        CommentRefKind::End => {
            let _ = write_empty(w, "w:commentRangeEnd", &[("w:id", &c.id)]);
            write_reference_run(w, &c.id);
        }
        // `Point` and any future kind fall back to a reference run.
        _ => write_reference_run(w, &c.id),
    }
}

/// Writes `<w:r><w:rPr><w:rStyle w:val="CommentReference"/></w:rPr><w:commentReference w:id=".."/></w:r>`.
fn write_reference_run<W: std::io::Write>(w: &mut Writer<W>, id: &str) {
    let _ = write_start(w, "w:r", &[]);
    let _ = write_start(w, "w:rPr", &[]);
    let _ = write_empty(w, "w:rStyle", &wval("CommentReference"));
    let _ = write_end(w, "w:rPr");
    let _ = write_empty(w, "w:commentReference", &[("w:id", id)]);
    let _ = write_end(w, "w:r");
}

/// Serializes `word/comments.xml` from the document's comments.
///
/// Each body [`Block`] is written as a `w:p`; its plain text is concatenated
/// into a single run (inline formatting inside comments is not preserved).
#[must_use]
pub(super) fn write_comments_xml(comments: &[Comment]) -> Vec<u8> {
    let mut out = String::from(concat!(
        "<?xml version=\"1.0\" encoding=\"UTF-8\" standalone=\"yes\"?>\n",
        "<w:comments xmlns:w=\"",
    ));
    out.push_str(NS_W);
    out.push_str("\">");
    for c in comments {
        out.push_str("<w:comment");
        attr(&mut out, "w:id", &c.id);
        if let Some(author) = &c.author {
            attr(&mut out, "w:author", author);
        }
        if let Some(date) = &c.date {
            attr(
                &mut out,
                "w:date",
                &date.to_rfc3339_opts(chrono::SecondsFormat::Secs, true),
            );
        }
        out.push('>');
        // Always emit at least one paragraph (a comment with no body is invalid).
        if c.body.is_empty() {
            out.push_str("<w:p/>");
        }
        for block in &c.body {
            out.push_str("<w:p><w:r><w:t xml:space=\"preserve\">");
            out.push_str(&escape(&block_text(block)));
            out.push_str("</w:t></w:r></w:p>");
        }
        out.push_str("</w:comment>");
    }
    out.push_str("</w:comments>");
    out.into_bytes()
}

/// Concatenates the plain text of a paragraph-like [`Block`].
fn block_text(block: &Block) -> String {
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

/// Appends ` name="value"` (value escaped) to `out`.
fn attr(out: &mut String, name: &str, value: &str) {
    out.push(' ');
    out.push_str(name);
    out.push_str("=\"");
    out.push_str(&escape(value));
    out.push('"');
}

/// Escapes XML text / attribute content.
fn escape(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    for c in s.chars() {
        match c {
            '&' => out.push_str("&amp;"),
            '<' => out.push_str("&lt;"),
            '>' => out.push_str("&gt;"),
            '"' => out.push_str("&quot;"),
            '\'' => out.push_str("&apos;"),
            _ => out.push(c),
        }
    }
    out
}
