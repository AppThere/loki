// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 AppThere Loki contributors

//! `content.xml` writer: the document body plus its automatic styles.

use loki_doc_model::content::block::{Block, StyledParagraph};
use loki_doc_model::content::inline::{Inline, NoteKind};
use loki_doc_model::content::table::{Row, Table};
use loki_doc_model::document::Document;
use loki_doc_model::style::catalog::StyleId;
use loki_doc_model::style::props::char_props::{
    CharProps, StrikethroughStyle, UnderlineStyle, VerticalAlign,
};

use super::auto::AutoStyles;
use super::xml::{attr, escape};

const HEADER: &str = concat!(
    "<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n",
    "<office:document-content",
    " xmlns:office=\"urn:oasis:names:tc:opendocument:xmlns:office:1.0\"",
    " xmlns:style=\"urn:oasis:names:tc:opendocument:xmlns:style:1.0\"",
    " xmlns:text=\"urn:oasis:names:tc:opendocument:xmlns:text:1.0\"",
    " xmlns:table=\"urn:oasis:names:tc:opendocument:xmlns:table:1.0\"",
    " xmlns:draw=\"urn:oasis:names:tc:opendocument:xmlns:drawing:1.0\"",
    " xmlns:fo=\"urn:oasis:names:tc:opendocument:xmlns:xsl-fo-compatible:1.0\"",
    " xmlns:xlink=\"http://www.w3.org/1999/xlink\"",
    " xmlns:svg=\"urn:oasis:names:tc:opendocument:xmlns:svg-compatible:1.0\"",
    " office:version=\"1.3\">",
);

/// Renders the whole `content.xml` for `doc`.
#[must_use]
pub(crate) fn content_xml(doc: &Document) -> String {
    let mut auto = AutoStyles::new();
    let mut body = String::new();
    for section in &doc.sections {
        for block in &section.blocks {
            write_block(&mut body, block, &mut auto);
        }
    }
    let mut out = String::with_capacity(body.len() + 1024);
    out.push_str(HEADER);
    out.push_str("<office:automatic-styles>");
    out.push_str(&auto.render());
    out.push_str("</office:automatic-styles>");
    out.push_str("<office:body><office:text>");
    out.push_str(&body);
    out.push_str("</office:text></office:body></office:document-content>");
    out
}

/// Writes a single block element.
fn write_block(out: &mut String, block: &Block, auto: &mut AutoStyles) {
    match block {
        Block::Para(inl) | Block::Plain(inl) => paragraph(out, None, inl, auto),
        Block::Heading(level, _, inl) => {
            let lvl = (*level).clamp(1, 6);
            out.push_str(&format!(
                "<text:h text:style-name=\"Heading{lvl}\" text:outline-level=\"{lvl}\">"
            ));
            write_inlines(out, inl, auto);
            out.push_str("</text:h>");
        }
        Block::StyledPara(sp) => styled_paragraph(out, sp, auto),
        Block::BlockQuote(blocks) | Block::Div(_, blocks) | Block::Figure(_, _, blocks) => {
            for b in blocks {
                write_block(out, b, auto);
            }
        }
        Block::BulletList(items) => list(out, false, items, auto),
        Block::OrderedList(_, items) => list(out, true, items, auto),
        Block::DefinitionList(items) => {
            for (term, defs) in items {
                paragraph(out, None, term, auto);
                for blocks in defs {
                    for b in blocks {
                        write_block(out, b, auto);
                    }
                }
            }
        }
        Block::CodeBlock(_, text) => {
            for line in text.split('\n') {
                out.push_str("<text:p>");
                out.push_str(&escape(line));
                out.push_str("</text:p>");
            }
        }
        Block::LineBlock(lines) => {
            out.push_str("<text:p>");
            for (i, line) in lines.iter().enumerate() {
                if i > 0 {
                    out.push_str("<text:line-break/>");
                }
                write_inlines(out, line, auto);
            }
            out.push_str("</text:p>");
        }
        Block::Table(t) => table(out, t, auto),
        // Generated / raw blocks are not round-tripped on export.
        _ => {}
    }
}

/// Writes a `<text:p>` with optional automatic style and inline content.
fn paragraph(out: &mut String, style: Option<&str>, inl: &[Inline], auto: &mut AutoStyles) {
    out.push_str("<text:p");
    if let Some(name) = style {
        attr(out, "text:style-name", name);
    }
    out.push('>');
    write_inlines(out, inl, auto);
    out.push_str("</text:p>");
}

/// Writes a `StyledParagraph`: references its named style, layering an automatic
/// style on top when it carries direct paragraph / character overrides.
fn styled_paragraph(out: &mut String, sp: &StyledParagraph, auto: &mut AutoStyles) {
    let base = sp.style_id.as_ref().map(StyleId::as_str);
    let pp = sp.direct_para_props.as_deref().cloned().unwrap_or_default();
    let cp = sp.direct_char_props.as_deref().cloned().unwrap_or_default();
    let name = if sp.direct_para_props.is_some() || sp.direct_char_props.is_some() {
        auto.para_style(base, &pp, &cp)
    } else {
        base.map(str::to_string)
    };
    paragraph(out, name.as_deref(), &sp.inlines, auto);
}

/// Writes a `<text:list>` from a list of items (each a block sequence).
fn list(out: &mut String, ordered: bool, items: &[Vec<Block>], auto: &mut AutoStyles) {
    let _ = ordered; // list numbering style is left to the consumer's defaults
    out.push_str("<text:list>");
    for item in items {
        out.push_str("<text:list-item>");
        if item.is_empty() {
            out.push_str("<text:p/>");
        } else {
            for b in item {
                write_block(out, b, auto);
            }
        }
        out.push_str("</text:list-item>");
    }
    out.push_str("</text:list>");
}

/// Writes a `<table:table>` (header rows, then bodies, then footer).
fn table(out: &mut String, t: &Table, auto: &mut AutoStyles) {
    out.push_str("<table:table>");
    let cols = t.col_specs.len().max(1);
    out.push_str(&format!(
        "<table:table-column table:number-columns-repeated=\"{cols}\"/>"
    ));
    for row in &t.head.rows {
        table_row(out, row, auto);
    }
    for body in &t.bodies {
        for row in body.head_rows.iter().chain(body.body_rows.iter()) {
            table_row(out, row, auto);
        }
    }
    for row in &t.foot.rows {
        table_row(out, row, auto);
    }
    out.push_str("</table:table>");
}

fn table_row(out: &mut String, row: &Row, auto: &mut AutoStyles) {
    out.push_str("<table:table-row>");
    for cell in &row.cells {
        out.push_str("<table:table-cell");
        if cell.col_span > 1 {
            attr(
                out,
                "table:number-columns-spanned",
                &cell.col_span.to_string(),
            );
        }
        if cell.row_span > 1 {
            attr(out, "table:number-rows-spanned", &cell.row_span.to_string());
        }
        out.push('>');
        if cell.blocks.is_empty() {
            out.push_str("<text:p/>");
        } else {
            for b in &cell.blocks {
                write_block(out, b, auto);
            }
        }
        out.push_str("</table:table-cell>");
        for _ in 1..cell.col_span {
            out.push_str("<table:covered-table-cell/>");
        }
    }
    out.push_str("</table:table-row>");
}

/// Writes a sequence of inline runs.
fn write_inlines(out: &mut String, inlines: &[Inline], auto: &mut AutoStyles) {
    for inl in inlines {
        write_inline(out, inl, auto);
    }
}

/// Writes one inline run.
fn write_inline(out: &mut String, inl: &Inline, auto: &mut AutoStyles) {
    match inl {
        Inline::Str(s) | Inline::Code(_, s) => out.push_str(&escape(s)),
        Inline::Space | Inline::SoftBreak => out.push(' '),
        Inline::LineBreak => out.push_str("<text:line-break/>"),
        Inline::Strong(c) => span(out, c, auto, set_bold),
        Inline::Emph(c) => span(out, c, auto, set_italic),
        Inline::Underline(c) => span(out, c, auto, set_underline),
        Inline::Strikeout(c) => span(out, c, auto, set_strikethrough),
        Inline::Superscript(c) => span(out, c, auto, set_superscript),
        Inline::Subscript(c) => span(out, c, auto, set_subscript),
        Inline::SmallCaps(c) => span(out, c, auto, set_small_caps),
        Inline::Span(_, c) | Inline::Quoted(_, c) | Inline::Cite(_, c) => {
            write_inlines(out, c, auto);
        }
        Inline::StyledRun(sr) => {
            let name = match sr.direct_props.as_deref() {
                Some(dp) => auto.text_style(dp),
                None => sr.style_id.as_ref().map(|s| s.as_str().to_string()),
            };
            wrap_span(out, name.as_deref(), &sr.content, auto);
        }
        Inline::Link(_, c, target) => {
            out.push_str("<text:a");
            attr(out, "xlink:href", &target.url);
            out.push('>');
            write_inlines(out, c, auto);
            out.push_str("</text:a>");
        }
        Inline::Note(kind, blocks) => note(out, *kind, blocks, auto),
        // Math, RawInline, Field, Comment, Bookmark, Image: no plain-text export.
        _ => {}
    }
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
fn span(
    out: &mut String,
    children: &[Inline],
    auto: &mut AutoStyles,
    f: impl FnOnce(&mut CharProps),
) {
    let mut cp = CharProps::default();
    f(&mut cp);
    let name = auto.text_style(&cp);
    wrap_span(out, name.as_deref(), children, auto);
}

/// Wraps `children` in a `<text:span text:style-name=...>`; emits them bare when
/// `name` is `None`.
fn wrap_span(out: &mut String, name: Option<&str>, children: &[Inline], auto: &mut AutoStyles) {
    if let Some(name) = name {
        out.push_str("<text:span");
        attr(out, "text:style-name", name);
        out.push('>');
        write_inlines(out, children, auto);
        out.push_str("</text:span>");
    } else {
        write_inlines(out, children, auto);
    }
}

/// Writes a footnote / endnote.
fn note(out: &mut String, kind: NoteKind, blocks: &[Block], auto: &mut AutoStyles) {
    let class = match kind {
        NoteKind::Endnote => "endnote",
        _ => "footnote",
    };
    out.push_str(&format!(
        "<text:note text:note-class=\"{class}\"><text:note-citation></text:note-citation>\
         <text:note-body>"
    ));
    for b in blocks {
        write_block(out, b, auto);
    }
    out.push_str("</text:note-body></text:note>");
}
