// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 AppThere Loki contributors

//! `meta.xml` rendering (ODF 1.3 §3.1 `office:meta`).
//!
//! Shared by ODT and ODS export so the part has one derivation: the ODT
//! writer supplies the full Dublin Core set from a [`Document`], the ODS
//! writer the title/creator pair its model carries. Both produce the same
//! element shape, so a fix or a namespace change lands once.
//!
//! [`Document`]: loki_doc_model::document::Document

/// Escapes XML text content / attribute values.
#[must_use]
pub(crate) fn escape(s: &str) -> String {
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

/// The metadata fields a `meta.xml` part can carry, borrowed from whichever
/// model the caller holds.
///
/// `creator` is written to both `dc:creator` and `meta:initial-creator`: ODF
/// distinguishes "who last saved" from "who created", and a model with a
/// single author field cannot say which, so it claims both rather than
/// leaving a foreign reader to guess.
#[derive(Debug, Default)]
pub(crate) struct MetaFields<'a> {
    /// `dc:title`.
    pub title: Option<&'a str>,
    /// `dc:creator` + `meta:initial-creator`.
    pub creator: Option<&'a str>,
    /// `dc:subject`.
    pub subject: Option<&'a str>,
    /// `dc:description`.
    pub description: Option<&'a str>,
    /// `meta:keyword`.
    pub keywords: Option<&'a str>,
    /// Extended Dublin Core, carried as `meta:user-defined` entries under
    /// their reserved `dcmi:` names (ODF has no native element for them).
    pub user_defined: Vec<(String, String)>,
}

/// Renders a complete `meta.xml` document for `fields`.
///
/// `office_version` is the package's `office:version` attribute (`"1.1"`,
/// `"1.2"`, or `"1.3"`).
#[must_use]
pub(crate) fn meta_xml_from(fields: &MetaFields<'_>, office_version: &str) -> String {
    let mut out = String::from(concat!(
        "<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n",
        "<office:document-meta",
        " xmlns:office=\"urn:oasis:names:tc:opendocument:xmlns:office:1.0\"",
        " xmlns:dc=\"http://purl.org/dc/elements/1.1/\"",
        " xmlns:meta=\"urn:oasis:names:tc:opendocument:xmlns:meta:1.0\"",
    ));
    out.push_str(&format!(
        " office:version=\"{office_version}\"><office:meta>"
    ));
    {
        let mut el = |tag: &str, val: Option<&str>| {
            if let Some(v) = val {
                out.push_str(&format!("<{tag}>{}</{tag}>", escape(v)));
            }
        };
        el("dc:title", fields.title);
        el("dc:creator", fields.creator);
        el("meta:initial-creator", fields.creator);
        el("dc:subject", fields.subject);
        el("dc:description", fields.description);
        el("meta:keyword", fields.keywords);
    }
    for (name, value) in &fields.user_defined {
        out.push_str(&format!(
            "<meta:user-defined meta:name=\"{}\">{}</meta:user-defined>",
            escape(name),
            escape(value),
        ));
    }
    out.push_str("</office:meta></office:document-meta>");
    out
}

#[cfg(test)]
#[path = "meta_write_tests.rs"]
mod tests;
