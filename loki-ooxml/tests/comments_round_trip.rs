// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 AppThere Loki contributors

//! DOCX comment round-trip: a commented range (`Inline::Comment` anchors) plus
//! its body (`word/comments.xml`) must survive export and re-import.

use std::io::Cursor;

use chrono::TimeZone;
use loki_doc_model::content::annotation::{Comment, CommentRef, CommentRefKind};
use loki_doc_model::content::block::Block;
use loki_doc_model::content::inline::Inline;
use loki_doc_model::document::Document;
use loki_doc_model::io::DocumentExport;
use loki_ooxml::DocxExport;
use loki_ooxml::docx::import::{DocxImportOptions, DocxImporter};

fn doc_with_comment() -> Document {
    // "Hello [commented]world[/commented]!" with comment id "0".
    let para = Block::Para(vec![
        Inline::Str("Hello ".to_string()),
        Inline::Comment(CommentRef::new("0", CommentRefKind::Start)),
        Inline::Str("world".to_string()),
        Inline::Comment(CommentRef::new("0", CommentRefKind::End)),
        Inline::Str("!".to_string()),
    ]);

    let mut comment = Comment::new("0");
    comment.author = Some("Reviewer".to_string());
    comment.date = Some(chrono::Utc.with_ymd_and_hms(2026, 6, 22, 9, 30, 0).unwrap());
    comment.body_raw = b"Please rephrase this.".to_vec();

    let mut doc = Document::new();
    doc.sections[0].blocks = vec![para];
    doc.comments = vec![comment];
    doc
}

fn anchors(doc: &Document) -> Vec<(String, CommentRefKind)> {
    let mut out = Vec::new();
    for section in &doc.sections {
        for block in &section.blocks {
            let inlines = match block {
                Block::Para(i) | Block::Plain(i) => i.as_slice(),
                Block::StyledPara(sp) => sp.inlines.as_slice(),
                _ => &[],
            };
            for i in inlines {
                if let Inline::Comment(c) = i {
                    out.push((c.id.clone(), c.kind));
                }
            }
        }
    }
    out
}

#[test]
fn comment_range_and_body_round_trip() {
    let doc = doc_with_comment();

    let mut buf = Cursor::new(Vec::new());
    DocxExport::export(&doc, &mut buf, ()).expect("export should succeed");
    let re = DocxImporter::new(DocxImportOptions::default())
        .run(Cursor::new(buf.into_inner()))
        .expect("re-import should succeed")
        .document;

    // The start/end anchors survive in the content flow.
    let got = anchors(&re);
    assert!(
        got.contains(&("0".to_string(), CommentRefKind::Start)),
        "comment start anchor must survive; got {got:?}"
    );
    assert!(
        got.contains(&("0".to_string(), CommentRefKind::End)),
        "comment end anchor must survive; got {got:?}"
    );

    // The comment body, author, and date survive in word/comments.xml.
    assert_eq!(re.comments.len(), 1, "one comment expected");
    let c = &re.comments[0];
    assert_eq!(c.id, "0");
    assert_eq!(c.author.as_deref(), Some("Reviewer"));
    assert_eq!(
        String::from_utf8_lossy(&c.body_raw),
        "Please rephrase this."
    );
    assert_eq!(
        c.date,
        Some(chrono::Utc.with_ymd_and_hms(2026, 6, 22, 9, 30, 0).unwrap())
    );
}
