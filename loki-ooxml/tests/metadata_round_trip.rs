// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 AppThere Loki contributors

//! DOCX document-metadata round-trip: core properties (`docProps/core.xml`)
//! and extended Dublin Core (`docProps/custom.xml`).

use std::io::Cursor;

use loki_doc_model::content::block::Block;
use loki_doc_model::content::inline::Inline;
use loki_doc_model::document::Document;
use loki_doc_model::io::DocumentExport;
use loki_doc_model::meta::core::DocumentMeta;
use loki_doc_model::meta::dublin_core::DublinCoreMeta;
use loki_doc_model::meta::language::LanguageTag;
use loki_ooxml::DocxExport;
use loki_ooxml::docx::import::{DocxImportOptions, DocxImporter};

fn doc_with_meta(meta: DocumentMeta) -> Document {
    let mut doc = Document::new();
    doc.meta = meta;
    doc.sections[0].blocks = vec![Block::Para(vec![Inline::Str("Body".to_string())])];
    doc
}

fn round_trip(doc: &Document) -> Document {
    let mut buf = Cursor::new(Vec::new());
    DocxExport::export(doc, &mut buf, ()).expect("export should succeed");
    DocxImporter::new(DocxImportOptions::default())
        .run(Cursor::new(buf.into_inner()))
        .expect("re-import should succeed")
        .document
}

#[test]
fn core_metadata_round_trips() {
    let meta = DocumentMeta {
        title: Some("Quarterly Report".into()),
        subject: Some("Finance".into()),
        keywords: Some("q3, revenue".into()),
        description: Some("An overview.".into()),
        creator: Some("Ada Lovelace".into()),
        last_modified_by: Some("Charles Babbage".into()),
        language: Some(LanguageTag::new("en-GB")),
        revision: Some(4),
        ..Default::default()
    };
    let re = round_trip(&doc_with_meta(meta)).meta;

    assert_eq!(re.title.as_deref(), Some("Quarterly Report"));
    assert_eq!(re.subject.as_deref(), Some("Finance"));
    assert_eq!(re.keywords.as_deref(), Some("q3, revenue"));
    assert_eq!(re.description.as_deref(), Some("An overview."));
    assert_eq!(re.creator.as_deref(), Some("Ada Lovelace"));
    assert_eq!(re.last_modified_by.as_deref(), Some("Charles Babbage"));
    assert_eq!(re.language.as_ref().map(|l| l.as_str()), Some("en-GB"));
    assert_eq!(re.revision, Some(4));
}

#[test]
fn extended_dublin_core_round_trips() {
    let dc = DublinCoreMeta {
        contributors: vec!["Editor One".into(), "Translator Two".into()],
        publisher: Some("AppThere Press".into()),
        rights: Some("© 2026 AppThere".into()),
        license: Some("https://creativecommons.org/licenses/by/4.0/".into()),
        identifier: Some("978-0-00-000000-0".into()),
        identifier_scheme: Some("ISBN".into()),
        dc_type: Some("Text".into()),
        format: Some(
            "application/vnd.openxmlformats-officedocument.wordprocessingml.document".into(),
        ),
        source: Some("Original manuscript".into()),
        relation: Some("Companion volume".into()),
        coverage: Some("2020–2026".into()),
        issued: Some("2026-06-22".into()),
        bibliographic_citation: Some("AppThere, Report (2026)".into()),
    };
    let meta = DocumentMeta {
        title: Some("With Extended DC".into()),
        dublin_core: dc.clone(),
        ..Default::default()
    };

    let re = round_trip(&doc_with_meta(meta)).meta.dublin_core;
    assert_eq!(
        re, dc,
        "all extended Dublin Core fields must survive round-trip"
    );
}
