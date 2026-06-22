// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 AppThere Loki contributors

//! `styles.xml` writer: the named style catalog, the page layout, and the
//! master page — which carries the section's page geometry and the
//! header/footer content.

use loki_doc_model::document::Document;
use loki_doc_model::layout::header_footer::HeaderFooter;
use loki_doc_model::layout::page::{PageLayout, PageOrientation};
use loki_doc_model::style::para_style::ParagraphStyle;

use super::auto::AutoStyles;
use super::content::{Cx, write_block};
use super::media::{Media, Rendered};
use super::para_props::emit_paragraph_properties;
use super::props::emit_text_properties;
use super::xml::{attr, escape, master_page_name, page_layout_name, pt};

const HEADER: &str = concat!(
    "<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n",
    "<office:document-styles",
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

/// Renders the whole `styles.xml` for `doc`, collecting any images embedded in
/// the master-page header/footer.
#[must_use]
pub(crate) fn styles_xml(doc: &Document) -> Rendered {
    // One page-layout + master-page per section so each section's geometry and
    // header/footer round-trip. A document with no sections falls back to a
    // single default master page.
    let layouts: Vec<PageLayout> = if doc.sections.is_empty() {
        vec![PageLayout::default()]
    } else {
        doc.sections.iter().map(|s| s.layout.clone()).collect()
    };

    // Render the master-page header/footer content first so its automatic
    // styles (and images) are collected before the automatic-styles section is
    // written. Header/footer styles MUST live in styles.xml, not content.xml.
    let mut cx = Cx {
        auto: AutoStyles::new(),
        media: Media::with_prefix("himg"),
    };
    let mut masters = String::new();
    let mut page_layouts = String::new();
    for (idx, layout) in layouts.iter().enumerate() {
        let mp_name = master_page_name(idx);
        let pl_name = page_layout_name(idx);
        masters.push_str(&render_master_page(&mp_name, &pl_name, layout, &mut cx));
        write_page_layout(&mut page_layouts, &pl_name, layout);
    }

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

    // ── Automatic styles: page layouts + header/footer styles ──────────────
    out.push_str("<office:automatic-styles>");
    out.push_str(&page_layouts);
    out.push_str(&cx.auto.render());
    out.push_str("</office:automatic-styles>");

    out.push_str("<office:master-styles>");
    out.push_str(&masters);
    out.push_str("</office:master-styles>");
    out.push_str("</office:document-styles>");

    Rendered {
        xml: out,
        media: cx.media.into_parts(),
    }
}

/// Builds one `<style:master-page>` element (named `mp_name`, referencing
/// page-layout `pl_name`), rendering each present header/footer variant into
/// ODF `<style:header…>` / `<style:footer…>`.
fn render_master_page(mp_name: &str, pl_name: &str, layout: &PageLayout, cx: &mut Cx) -> String {
    let mut m = String::from("<style:master-page");
    attr(&mut m, "style:name", mp_name);
    attr(&mut m, "style:page-layout-name", pl_name);
    m.push('>');
    write_hf(&mut m, "style:header", layout.header.as_ref(), cx);
    write_hf(&mut m, "style:footer", layout.footer.as_ref(), cx);
    write_hf(
        &mut m,
        "style:header-first",
        layout.header_first.as_ref(),
        cx,
    );
    write_hf(
        &mut m,
        "style:footer-first",
        layout.footer_first.as_ref(),
        cx,
    );
    write_hf(&mut m, "style:header-left", layout.header_even.as_ref(), cx);
    write_hf(&mut m, "style:footer-left", layout.footer_even.as_ref(), cx);
    m.push_str("</style:master-page>");
    m
}

/// Writes one `<style:header…>` / `<style:footer…>` element with its paragraphs.
fn write_hf(out: &mut String, tag: &str, hf: Option<&HeaderFooter>, cx: &mut Cx) {
    let Some(hf) = hf else {
        return;
    };
    out.push_str(&format!("<{tag}>"));
    if hf.blocks.is_empty() {
        out.push_str("<text:p/>");
    } else {
        for b in &hf.blocks {
            write_block(out, b, cx);
        }
    }
    out.push_str(&format!("</{tag}>"));
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

/// Writes the `<style:page-layout style:name="{pl_name}">` element from `layout`.
fn write_page_layout(out: &mut String, pl_name: &str, layout: &PageLayout) {
    out.push_str("<style:page-layout");
    attr(out, "style:name", pl_name);
    out.push_str("><style:page-layout-properties");
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
    out.push_str("/>");
    // Declaring header/footer styles makes apps reserve the space and render the
    // master-page content; omit them when the document has none.
    if layout.header.is_some() || layout.header_first.is_some() || layout.header_even.is_some() {
        out.push_str("<style:header-style/>");
    }
    if layout.footer.is_some() || layout.footer_first.is_some() || layout.footer_even.is_some() {
        out.push_str("<style:footer-style/>");
    }
    out.push_str("</style:page-layout>");
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
