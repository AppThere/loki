// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 AppThere Loki contributors

//! `content.xml` writer: the document body, its automatic styles, and the
//! embedded images it references. Inline-run serialisation lives in
//! [`super::inlines`].

use loki_doc_model::content::block::{Block, StyledParagraph};
use loki_doc_model::content::table::{Row, Table};
use loki_doc_model::document::Document;
use loki_doc_model::style::catalog::StyleId;

use super::auto::AutoStyles;
use super::inlines::write_inlines;
use super::media::{Media, MediaPart};
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

/// Shared writer state threaded through the body: the automatic-style collector
/// and the embedded-image collector.
pub(super) struct Cx {
    pub(super) auto: AutoStyles,
    pub(super) media: Media,
}

/// The rendered `content.xml` together with the image parts it references.
pub(crate) struct Content {
    pub(crate) xml: String,
    pub(crate) media: Vec<MediaPart>,
}

/// Renders the whole `content.xml` for `doc`, collecting any embedded images.
#[must_use]
pub(crate) fn content_xml(doc: &Document) -> Content {
    let mut cx = Cx {
        auto: AutoStyles::new(),
        media: Media::new(),
    };
    let mut body = String::new();
    for section in &doc.sections {
        for block in &section.blocks {
            write_block(&mut body, block, &mut cx);
        }
    }
    let mut out = String::with_capacity(body.len() + 1024);
    out.push_str(HEADER);
    out.push_str("<office:automatic-styles>");
    out.push_str(&cx.auto.render());
    out.push_str("</office:automatic-styles>");
    out.push_str("<office:body><office:text>");
    out.push_str(&body);
    out.push_str("</office:text></office:body></office:document-content>");
    Content {
        xml: out,
        media: cx.media.into_parts(),
    }
}

/// Writes a single block element.
pub(super) fn write_block(out: &mut String, block: &Block, cx: &mut Cx) {
    match block {
        Block::Para(inl) | Block::Plain(inl) => paragraph(out, None, inl, cx),
        Block::Heading(level, _, inl) => {
            let lvl = (*level).clamp(1, 6);
            out.push_str(&format!(
                "<text:h text:style-name=\"Heading{lvl}\" text:outline-level=\"{lvl}\">"
            ));
            write_inlines(out, inl, cx);
            out.push_str("</text:h>");
        }
        Block::StyledPara(sp) => styled_paragraph(out, sp, cx),
        Block::BlockQuote(blocks) | Block::Div(_, blocks) | Block::Figure(_, _, blocks) => {
            for b in blocks {
                write_block(out, b, cx);
            }
        }
        Block::BulletList(items) | Block::OrderedList(_, items) => list(out, items, cx),
        Block::DefinitionList(items) => {
            for (term, defs) in items {
                paragraph(out, None, term, cx);
                for blocks in defs {
                    for b in blocks {
                        write_block(out, b, cx);
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
                write_inlines(out, line, cx);
            }
            out.push_str("</text:p>");
        }
        Block::Table(t) => table(out, t, cx),
        // Preserve the rendered text of generated blocks (TOC / index) rather
        // than dropping it.
        Block::TableOfContents(toc) => {
            for b in &toc.body {
                write_block(out, b, cx);
            }
        }
        Block::Index(idx) => {
            for b in &idx.body {
                write_block(out, b, cx);
            }
        }
        // Raw / notes blocks have no faithful ODF body representation.
        _ => {}
    }
}

/// Writes a `<text:p>` with optional automatic style and inline content.
fn paragraph(
    out: &mut String,
    style: Option<&str>,
    inl: &[loki_doc_model::content::inline::Inline],
    cx: &mut Cx,
) {
    out.push_str("<text:p");
    if let Some(name) = style {
        attr(out, "text:style-name", name);
    }
    out.push('>');
    write_inlines(out, inl, cx);
    out.push_str("</text:p>");
}

/// Writes a `StyledParagraph`: references its named style, layering an automatic
/// style on top when it carries direct paragraph / character overrides.
fn styled_paragraph(out: &mut String, sp: &StyledParagraph, cx: &mut Cx) {
    let base = sp.style_id.as_ref().map(StyleId::as_str);
    let pp = sp.direct_para_props.as_deref().cloned().unwrap_or_default();
    let cp = sp.direct_char_props.as_deref().cloned().unwrap_or_default();
    let name = if sp.direct_para_props.is_some() || sp.direct_char_props.is_some() {
        cx.auto.para_style(base, &pp, &cp)
    } else {
        base.map(str::to_string)
    };
    paragraph(out, name.as_deref(), &sp.inlines, cx);
}

/// Writes a `<text:list>` from a list of items (each a block sequence).
fn list(out: &mut String, items: &[Vec<Block>], cx: &mut Cx) {
    out.push_str("<text:list>");
    for item in items {
        out.push_str("<text:list-item>");
        if item.is_empty() {
            out.push_str("<text:p/>");
        } else {
            for b in item {
                write_block(out, b, cx);
            }
        }
        out.push_str("</text:list-item>");
    }
    out.push_str("</text:list>");
}

/// Writes a `<table:table>` (header rows, then bodies, then footer).
fn table(out: &mut String, t: &Table, cx: &mut Cx) {
    out.push_str("<table:table>");
    let cols = t.col_specs.len().max(1);
    out.push_str(&format!(
        "<table:table-column table:number-columns-repeated=\"{cols}\"/>"
    ));
    for row in &t.head.rows {
        table_row(out, row, cx);
    }
    for body in &t.bodies {
        for row in body.head_rows.iter().chain(body.body_rows.iter()) {
            table_row(out, row, cx);
        }
    }
    for row in &t.foot.rows {
        table_row(out, row, cx);
    }
    out.push_str("</table:table>");
}

fn table_row(out: &mut String, row: &Row, cx: &mut Cx) {
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
                write_block(out, b, cx);
            }
        }
        out.push_str("</table:table-cell>");
        for _ in 1..cell.col_span {
            out.push_str("<table:covered-table-cell/>");
        }
    }
    out.push_str("</table:table-row>");
}
