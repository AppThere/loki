// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 AppThere Loki contributors

//! Builds the `<metadata>` element of the EPUB package document from the
//! document's [`DocumentMeta`] and [`DublinCoreMeta`].
//!
//! EPUB 3.3 §5.4 requires `dc:identifier`, `dc:title`, `dc:language`, and a
//! `dcterms:modified` property. All other Dublin Core fields are optional.

use loki_doc_model::meta::{DocumentMeta, DublinCoreMeta};

use crate::xml::{escape_attr, escape_text};

/// Renders the `<metadata>` block.
///
/// `identifier` is the publication's unique identifier (already synthesised by
/// the caller when absent) and is referenced by the package
/// `unique-identifier` attribute via the fixed id `pub-id`. `modified_iso` is
/// the `dcterms:modified` timestamp in `CCYY-MM-DDThh:mm:ssZ` form.
#[must_use]
pub fn build_metadata(meta: &DocumentMeta, identifier: &str, modified_iso: &str) -> String {
    let dc = &meta.dublin_core;
    let mut m = String::new();
    m.push_str("  <metadata xmlns:dc=\"http://purl.org/dc/elements/1.1/\">\n");

    // ── Required elements ────────────────────────────────────────────────────
    push_el(&mut m, "dc:identifier", identifier, Some(("id", "pub-id")));
    if let Some(scheme) = dc.identifier_scheme.as_deref() {
        push_refine(&mut m, "pub-id", "identifier-type", scheme);
    }

    let title = meta.title.as_deref().unwrap_or("Untitled");
    push_el(&mut m, "dc:title", title, None);

    let lang = meta
        .language
        .as_ref()
        .map(|l| l.as_str().to_string())
        .unwrap_or_else(|| "en".to_string());
    push_el(&mut m, "dc:language", &lang, None);

    // dcterms:modified is mandatory and must be a single UTC timestamp.
    m.push_str(&format!(
        "    <meta property=\"dcterms:modified\">{}</meta>\n",
        escape_text(modified_iso)
    ));

    // ── Recommended / optional Dublin Core ───────────────────────────────────
    push_opt(&mut m, "dc:creator", meta.creator.as_deref());
    for contributor in &dc.contributors {
        push_el(&mut m, "dc:contributor", contributor, None);
    }
    push_opt(&mut m, "dc:description", meta.description.as_deref());
    push_opt(&mut m, "dc:publisher", dc.publisher.as_deref());
    push_opt(&mut m, "dc:rights", dc.rights.as_deref());
    push_opt(&mut m, "dc:source", dc.source.as_deref());
    push_opt(&mut m, "dc:relation", dc.relation.as_deref());
    push_opt(&mut m, "dc:coverage", dc.coverage.as_deref());
    push_opt(&mut m, "dc:type", Some(dc.dc_type_or_default()));
    push_opt(&mut m, "dc:format", dc.format.as_deref());

    // Subjects: split the comma-separated keyword list into discrete subjects.
    if let Some(subject) = meta.subject.as_deref() {
        push_el(&mut m, "dc:subject", subject, None);
    }
    if let Some(keywords) = meta.keywords.as_deref() {
        for kw in keywords.split(',').map(str::trim).filter(|s| !s.is_empty()) {
            push_el(&mut m, "dc:subject", kw, None);
        }
    }

    // Publication date: prefer the explicit issued date, else the created date.
    if let Some(issued) = dc.issued.as_deref() {
        push_el(&mut m, "dc:date", issued, None);
    } else if let Some(created) = meta.created {
        push_el(
            &mut m,
            "dc:date",
            &created.format("%Y-%m-%d").to_string(),
            None,
        );
    }

    if let Some(license) = dc.license.as_deref() {
        m.push_str(&format!(
            "    <link rel=\"cc:license\" href=\"{}\"/>\n",
            escape_attr(license)
        ));
    }
    if let Some(citation) = dc.bibliographic_citation.as_deref() {
        m.push_str(&format!(
            "    <meta property=\"dcterms:bibliographicCitation\">{}</meta>\n",
            escape_text(citation)
        ));
    }

    m.push_str("  </metadata>\n");
    m
}

fn push_opt(out: &mut String, tag: &str, value: Option<&str>) {
    if let Some(v) = value.filter(|s| !s.is_empty()) {
        push_el(out, tag, v, None);
    }
}

fn push_el(out: &mut String, tag: &str, value: &str, attr: Option<(&str, &str)>) {
    match attr {
        Some((name, val)) => out.push_str(&format!(
            "    <{tag} {name}=\"{aval}\">{text}</{tag}>\n",
            tag = tag,
            name = name,
            aval = escape_attr(val),
            text = escape_text(value),
        )),
        None => out.push_str(&format!(
            "    <{tag}>{text}</{tag}>\n",
            tag = tag,
            text = escape_text(value),
        )),
    }
}

fn push_refine(out: &mut String, refines_id: &str, property: &str, value: &str) {
    out.push_str(&format!(
        "    <meta refines=\"#{id}\" property=\"{prop}\">{text}</meta>\n",
        id = refines_id,
        prop = property,
        text = escape_text(value),
    ));
}

/// Returns `true` if any optional Dublin Core field beyond the required set is
/// present. Exposed for callers/tests that want to assert metadata richness.
#[must_use]
pub fn has_extended_metadata(meta: &DocumentMeta) -> bool {
    meta.creator.is_some()
        || meta.description.is_some()
        || meta.subject.is_some()
        || meta.keywords.is_some()
        || !DublinCoreMeta::is_empty(&meta.dublin_core)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn includes_required_elements() {
        let mut meta = DocumentMeta::default();
        meta.title = Some("Test".into());
        let xml = build_metadata(&meta, "urn:uuid:abc", "2026-01-01T00:00:00Z");
        assert!(xml.contains("<dc:identifier id=\"pub-id\">urn:uuid:abc</dc:identifier>"));
        assert!(xml.contains("<dc:title>Test</dc:title>"));
        assert!(xml.contains("<dc:language>en</dc:language>"));
        assert!(xml.contains("dcterms:modified\">2026-01-01T00:00:00Z"));
        assert!(xml.contains("<dc:type>Text</dc:type>"));
    }

    #[test]
    fn emits_publisher_and_keywords() {
        let mut meta = DocumentMeta::default();
        meta.title = Some("Book".into());
        meta.keywords = Some("rust, pdf, epub".into());
        meta.dublin_core.publisher = Some("AppThere".into());
        let xml = build_metadata(&meta, "id", "2026-01-01T00:00:00Z");
        assert!(xml.contains("<dc:publisher>AppThere</dc:publisher>"));
        assert!(xml.contains("<dc:subject>rust</dc:subject>"));
        assert!(xml.contains("<dc:subject>epub</dc:subject>"));
    }
}
