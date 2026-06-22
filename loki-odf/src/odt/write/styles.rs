// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 AppThere Loki contributors

//! `styles.xml` writer: the named style catalog plus the page layout and
//! master page (which carry the section's page size and margins).

use loki_doc_model::document::Document;
use loki_doc_model::layout::page::{PageLayout, PageOrientation};
use loki_doc_model::style::para_style::ParagraphStyle;

use super::para_props::emit_paragraph_properties;
use super::props::emit_text_properties;
use super::xml::{attr, escape, pt};

const HEADER: &str = concat!(
    "<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n",
    "<office:document-styles",
    " xmlns:office=\"urn:oasis:names:tc:opendocument:xmlns:office:1.0\"",
    " xmlns:style=\"urn:oasis:names:tc:opendocument:xmlns:style:1.0\"",
    " xmlns:text=\"urn:oasis:names:tc:opendocument:xmlns:text:1.0\"",
    " xmlns:fo=\"urn:oasis:names:tc:opendocument:xmlns:xsl-fo-compatible:1.0\"",
    " office:version=\"1.3\">",
);

/// Renders the whole `styles.xml` for `doc`.
#[must_use]
pub(crate) fn styles_xml(doc: &Document) -> String {
    let mut out = String::new();
    out.push_str(HEADER);

    // ── Named styles (the catalog) ─────────────────────────────────────────
    out.push_str("<office:styles>");
    for (id, style) in &doc.styles.paragraph_styles {
        write_paragraph_style(&mut out, id.as_str(), style);
    }
    for (id, style) in &doc.styles.character_styles {
        out.push_str("<style:style");
        attr(&mut out, "style:name", id.as_str());
        if let Some(name) = &style.display_name {
            attr(&mut out, "style:display-name", name);
        }
        attr(&mut out, "style:family", "text");
        if let Some(parent) = &style.parent {
            attr(&mut out, "style:parent-style-name", parent.as_str());
        }
        out.push('>');
        out.push_str(&emit_text_properties(&style.char_props));
        out.push_str("</style:style>");
    }
    out.push_str("</office:styles>");

    // ── Page layout (geometry) ─────────────────────────────────────────────
    let layout = doc
        .sections
        .first()
        .map(|s| &s.layout)
        .cloned()
        .unwrap_or_default();
    out.push_str("<office:automatic-styles>");
    write_page_layout(&mut out, &layout);
    out.push_str("</office:automatic-styles>");

    out.push_str(
        "<office:master-styles>\
         <style:master-page style:name=\"Standard\" style:page-layout-name=\"PL1\"/>\
         </office:master-styles>",
    );
    out.push_str("</office:document-styles>");
    out
}

/// Writes a `<style:style style:family="paragraph">` for a catalog style.
fn write_paragraph_style(out: &mut String, id: &str, style: &ParagraphStyle) {
    out.push_str("<style:style");
    attr(out, "style:name", id);
    if let Some(name) = &style.display_name {
        attr(out, "style:display-name", name);
    }
    attr(out, "style:family", "paragraph");
    if let Some(parent) = &style.parent {
        attr(out, "style:parent-style-name", parent.as_str());
    }
    if let Some(next) = &style.next_style_id {
        attr(out, "style:next-style-name", next);
    }
    out.push('>');
    out.push_str(&emit_paragraph_properties(&style.para_props));
    out.push_str(&emit_text_properties(&style.char_props));
    out.push_str("</style:style>");
}

/// Writes the `<style:page-layout style:name="PL1">` element from `layout`.
fn write_page_layout(out: &mut String, layout: &PageLayout) {
    out.push_str("<style:page-layout style:name=\"PL1\"><style:page-layout-properties");
    attr(out, "fo:page-width", &pt(layout.page_size.width));
    attr(out, "fo:page-height", &pt(layout.page_size.height));
    attr(out, "fo:margin-top", &pt(layout.margins.top));
    attr(out, "fo:margin-bottom", &pt(layout.margins.bottom));
    attr(out, "fo:margin-left", &pt(layout.margins.left));
    attr(out, "fo:margin-right", &pt(layout.margins.right));
    let orient = match layout.orientation {
        PageOrientation::Landscape => "landscape",
        PageOrientation::Portrait => "portrait",
    };
    attr(out, "style:print-orientation", orient);
    out.push_str("/></style:page-layout>");
}

/// Renders `meta.xml` for `doc` (Dublin Core core properties).
#[must_use]
pub(crate) fn meta_xml(doc: &Document) -> String {
    let mut out = String::from(concat!(
        "<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n",
        "<office:document-meta",
        " xmlns:office=\"urn:oasis:names:tc:opendocument:xmlns:office:1.0\"",
        " xmlns:dc=\"http://purl.org/dc/elements/1.1/\"",
        " xmlns:meta=\"urn:oasis:names:tc:opendocument:xmlns:meta:1.0\"",
        " office:version=\"1.3\"><office:meta>",
    ));
    let m = &doc.meta;
    let mut el = |tag: &str, val: &Option<String>| {
        if let Some(v) = val {
            out.push_str(&format!("<{tag}>{}</{tag}>", escape(v)));
        }
    };
    el("dc:title", &m.title);
    el("dc:creator", &m.creator);
    el("meta:initial-creator", &m.creator);
    el("dc:subject", &m.subject);
    el("dc:description", &m.description);
    el("meta:keyword", &m.keywords);
    out.push_str("</office:meta></office:document-meta>");
    out
}
