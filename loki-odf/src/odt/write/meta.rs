// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 AppThere Loki contributors

//! `meta.xml` writer (Dublin Core core + extended properties). Split out of
//! `styles.rs` (file-ceiling pass).

use loki_doc_model::document::Document;

use super::xml::escape;

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
    {
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
    }
    // Extended Dublin Core has no native office:meta element; carry each field
    // as a meta:user-defined entry under its reserved dcmi: name so it
    // round-trips.
    for (name, value) in m.dublin_core.to_named_pairs() {
        out.push_str(&format!(
            "<meta:user-defined meta:name=\"{}\">{}</meta:user-defined>",
            escape(&name),
            escape(&value),
        ));
    }
    out.push_str("</office:meta></office:document-meta>");
    out
}
