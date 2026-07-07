// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 AppThere Loki contributors

//! DOCX tracked-change round-trip: `w:ins` / `w:del` runs carrying a
//! `RevisionMark` (kind + author + date) must survive export and re-import
//! (Review tab, 4a.2).

use std::io::Cursor;

use loki_doc_model::content::attr::NodeAttr;
use loki_doc_model::content::block::Block;
use loki_doc_model::content::inline::{Inline, StyledRun};
use loki_doc_model::document::Document;
use loki_doc_model::io::DocumentExport;
use loki_doc_model::style::props::char_props::CharProps;
use loki_doc_model::style::props::revision::{RevisionKind, RevisionMark};
use loki_ooxml::DocxExport;
use loki_ooxml::docx::import::{DocxImportOptions, DocxImporter};

fn export_import(doc: &Document) -> Document {
    let mut buf = Cursor::new(Vec::new());
    DocxExport::export(doc, &mut buf, ()).expect("export");
    DocxImporter::new(DocxImportOptions::default())
        .run(Cursor::new(buf.into_inner()))
        .expect("re-import")
        .document
}

fn tracked_run(kind: RevisionKind, author: &str, date: &str, text: &str) -> Inline {
    let mut mark = RevisionMark::new(kind).with_author(author);
    mark.date = Some(date.to_string());
    Inline::StyledRun(StyledRun {
        style_id: None,
        direct_props: Some(Box::new(CharProps {
            revision: Some(mark),
            ..CharProps::default()
        })),
        content: vec![Inline::Str(text.into())],
        attr: NodeAttr::default(),
    })
}

/// Finds the first run carrying a revision of `kind` and returns its `(author,
/// date, text)`.
fn find_revision(
    doc: &Document,
    kind: RevisionKind,
) -> Option<(Option<String>, Option<String>, String)> {
    let inlines = match &doc.sections[0].blocks[0] {
        Block::Para(i) => i,
        Block::StyledPara(p) => &p.inlines,
        _ => return None,
    };
    inlines.iter().find_map(|i| match i {
        Inline::StyledRun(run) => {
            let rev = run.direct_props.as_ref()?.revision.as_ref()?;
            if rev.kind != kind {
                return None;
            }
            let text: String = run
                .content
                .iter()
                .map(|c| match c {
                    Inline::Str(s) => s.as_str(),
                    _ => "",
                })
                .collect();
            Some((rev.author.clone(), rev.date.clone(), text))
        }
        _ => None,
    })
}

#[test]
fn tracked_insertion_and_deletion_round_trip() {
    let mut doc = Document::new();
    doc.sections[0].blocks = vec![Block::Para(vec![
        Inline::Str("Kept ".into()),
        tracked_run(
            RevisionKind::Insertion,
            "Ada",
            "2026-07-07T12:00:00Z",
            "added",
        ),
        tracked_run(
            RevisionKind::Deletion,
            "Bob",
            "2026-07-07T13:00:00Z",
            "removed",
        ),
    ])];

    let back = export_import(&doc);

    let ins = find_revision(&back, RevisionKind::Insertion).expect("insertion survives");
    assert_eq!(
        ins,
        (
            Some("Ada".into()),
            Some("2026-07-07T12:00:00Z".into()),
            "added".into()
        )
    );

    let del = find_revision(&back, RevisionKind::Deletion).expect("deletion survives");
    assert_eq!(
        del,
        (
            Some("Bob".into()),
            Some("2026-07-07T13:00:00Z".into()),
            "removed".into()
        )
    );

    // The deleted run's text survives too (via w:delText), and the kept text is intact.
    assert!(back.has_tracked_changes());
}
