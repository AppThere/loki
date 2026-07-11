// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 AppThere Loki contributors

//! ODT tracked-change round-trip: a run carrying a `RevisionMark` exports as an
//! ODF change region (`text:changed-region` + `text:change-*` milestones) and
//! re-imports with its kind, author, date, and text intact (Review tab, 4a.2).

use std::io::Cursor;

use loki_doc_model::content::attr::NodeAttr;
use loki_doc_model::content::block::{Block, StyledParagraph};
use loki_doc_model::content::inline::{Inline, StyledRun};
use loki_doc_model::document::Document;
use loki_doc_model::io::{DocumentExport, DocumentImport};
use loki_doc_model::style::props::char_props::CharProps;
use loki_doc_model::style::props::revision::{RevisionKind, RevisionMark};
use loki_odf::odt::export::OdtExport;
use loki_odf::odt::import::{OdtImport, OdtImportOptions};

fn round_trip(doc: &Document) -> Document {
    let mut buf = Cursor::new(Vec::new());
    OdtExport::export(doc, &mut buf, Default::default()).expect("export");
    OdtImport::import(Cursor::new(buf.into_inner()), OdtImportOptions::default())
        .expect("re-import")
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

/// Finds the first run carrying a revision of `kind`, returning `(author, date,
/// text)`.
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

    let back = round_trip(&doc);

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

    assert!(back.has_tracked_changes());
}

/// A tracked deletion of the *paragraph mark* (¶) — `direct_char_props.revision`
/// on the block — exports as an end-of-paragraph `text:change` point whose
/// deletion region stows only the paragraph break (an empty `text:p`), and
/// re-imports onto the paragraph, not as a struck text run (4a.2 polish tail).
#[test]
fn tracked_paragraph_mark_deletion_round_trips() {
    let mut mark = RevisionMark::new(RevisionKind::Deletion).with_author("Cara");
    mark.date = Some("2026-07-08T09:00:00Z".to_string());
    let mut doc = Document::new();
    doc.sections[0].blocks = vec![
        Block::StyledPara(StyledParagraph {
            style_id: None,
            direct_para_props: None,
            direct_char_props: Some(Box::new(CharProps {
                revision: Some(mark),
                ..CharProps::default()
            })),
            inlines: vec![Inline::Str("First".into())],
            attr: NodeAttr::default(),
        }),
        Block::Para(vec![Inline::Str("Second".into())]),
    ];

    let back = round_trip(&doc);

    let Block::StyledPara(first) = &back.sections[0].blocks[0] else {
        panic!("first block must stay a styled paragraph");
    };
    let rev = first
        .direct_char_props
        .as_ref()
        .and_then(|cp| cp.revision.as_ref())
        .expect("¶-mark deletion survives on the paragraph");
    assert_eq!(rev.kind, RevisionKind::Deletion);
    assert_eq!(rev.author.as_deref(), Some("Cara"));
    assert_eq!(rev.date.as_deref(), Some("2026-07-08T09:00:00Z"));
    // The ¶ deletion must not be re-materialised as a struck (empty) text run.
    assert!(
        find_revision(&back, RevisionKind::Deletion).is_none(),
        "no struck run stands in for the paragraph break"
    );
    assert!(back.has_tracked_changes());
}
