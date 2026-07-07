// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 AppThere Loki contributors

//! `content.xml` writer: the document body, its automatic styles, and the
//! embedded images it references. Inline-run serialisation lives in
//! [`super::inlines`].

use loki_doc_model::content::block::{Block, StyledParagraph};
use loki_doc_model::document::Document;
use loki_doc_model::style::catalog::StyleId;
use loki_doc_model::style::props::char_props::CharProps;
use loki_doc_model::style::props::para_props::ParaProps;

use super::auto::AutoStyles;
use super::inlines::write_inlines;
use super::media::{MathPart, Media, Rendered};
use super::page_styles::resolve_page_style_names;
use super::tables::table;
use super::xml::{attr, escape};

const HEADER: &str = concat!(
    "<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n",
    "<office:document-content",
    " xmlns:office=\"urn:oasis:names:tc:opendocument:xmlns:office:1.0\"",
    " xmlns:dc=\"http://purl.org/dc/elements/1.1/\"",
    " xmlns:style=\"urn:oasis:names:tc:opendocument:xmlns:style:1.0\"",
    " xmlns:text=\"urn:oasis:names:tc:opendocument:xmlns:text:1.0\"",
    " xmlns:table=\"urn:oasis:names:tc:opendocument:xmlns:table:1.0\"",
    " xmlns:draw=\"urn:oasis:names:tc:opendocument:xmlns:drawing:1.0\"",
    " xmlns:fo=\"urn:oasis:names:tc:opendocument:xmlns:xsl-fo-compatible:1.0\"",
    " xmlns:xlink=\"http://www.w3.org/1999/xlink\"",
    " xmlns:svg=\"urn:oasis:names:tc:opendocument:xmlns:svg-compatible:1.0\"",
    " office:version=\"1.3\">",
);

/// Shared writer state threaded through the body: the automatic-style collector,
/// the embedded-image collector, and a comment lookup (id → body) for emitting
/// `office:annotation` content at the comment's start anchor.
pub(super) struct Cx {
    pub(super) auto: AutoStyles,
    pub(super) media: Media,
    pub(super) comments:
        std::collections::HashMap<String, loki_doc_model::content::annotation::Comment>,
    /// Embedded formula objects collected from `Inline::Math` runs.
    pub(super) objects: Vec<MathPart>,
}

/// Renders the whole `content.xml` for `doc`, collecting any embedded images.
#[must_use]
pub(crate) fn content_xml(doc: &Document) -> Rendered {
    let mut cx = Cx {
        auto: AutoStyles::new(),
        media: Media::new(),
        comments: doc
            .comments
            .iter()
            .map(|c| (c.id.clone(), c.clone()))
            .collect(),
        objects: Vec::new(),
    };
    // The section→master-page names, honouring the stored `page_style` refs so a
    // named page style round-trips (must agree with `styles.xml`).
    let names = resolve_page_style_names(doc);
    let mut body = String::new();
    for (idx, section) in doc.sections.iter().enumerate() {
        // Sections after the first trigger a page-geometry change by attaching
        // `style:master-page-name` to their first paragraph (ODF has no explicit
        // section element). The first section uses the initial master page.
        let master = (idx > 0).then(|| names.section_master[idx].clone());
        match (master.as_deref(), section.blocks.first()) {
            (Some(mp), Some(first)) => {
                write_block_with_master(&mut body, first, mp, &mut cx);
                for block in &section.blocks[1..] {
                    write_block(&mut body, block, &mut cx);
                }
            }
            (Some(mp), None) => {
                // Empty section: emit a carrier paragraph so the break survives.
                let style = cx.auto.para_style_master(
                    None,
                    &ParaProps::default(),
                    &CharProps::default(),
                    mp,
                );
                body.push_str(&format!("<text:p text:style-name=\"{style}\"/>"));
            }
            _ => {
                for block in &section.blocks {
                    write_block(&mut body, block, &mut cx);
                }
            }
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
    Rendered {
        xml: out,
        media: cx.media.into_parts(),
        objects: cx.objects,
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

/// Writes the first block of a section, attaching `style:master-page-name` so
/// the page-geometry change round-trips. Paragraph-like blocks carry the
/// reference directly; for any other block (table, list, …) a minimal carrier
/// paragraph is injected before it, since only paragraphs hold the attribute.
fn write_block_with_master(out: &mut String, block: &Block, master: &str, cx: &mut Cx) {
    match block {
        Block::Para(inl) | Block::Plain(inl) => {
            let style = cx.auto.para_style_master(
                None,
                &ParaProps::default(),
                &CharProps::default(),
                master,
            );
            paragraph(out, Some(&style), inl, cx);
        }
        Block::StyledPara(sp) => {
            let base = sp.style_id.as_ref().map(StyleId::as_str);
            let pp = sp.direct_para_props.as_deref().cloned().unwrap_or_default();
            let cp = sp.direct_char_props.as_deref().cloned().unwrap_or_default();
            let style = cx.auto.para_style_master(base, &pp, &cp, master);
            paragraph(out, Some(&style), &sp.inlines, cx);
        }
        Block::Heading(level, _, inl) => {
            let lvl = (*level).clamp(1, 6);
            let parent = format!("Heading{lvl}");
            let style = cx.auto.para_style_master(
                Some(&parent),
                &ParaProps::default(),
                &CharProps::default(),
                master,
            );
            out.push_str(&format!(
                "<text:h text:style-name=\"{style}\" text:outline-level=\"{lvl}\">"
            ));
            write_inlines(out, inl, cx);
            out.push_str("</text:h>");
        }
        other => {
            let style = cx.auto.para_style_master(
                None,
                &ParaProps::default(),
                &CharProps::default(),
                master,
            );
            out.push_str(&format!("<text:p text:style-name=\"{style}\"/>"));
            write_block(out, other, cx);
        }
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
