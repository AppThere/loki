// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 AppThere Loki contributors

//! Reader for `word/comments.xml` (ECMA-376 §17.13.4.2): parses each
//! `w:comment` into a [`Comment`], with its author, date, and block body
//! (one [`Block::Para`] per `w:p`, preserving multiple paragraphs).

use chrono::DateTime;
use quick_xml::Reader;
use quick_xml::events::Event;

use loki_doc_model::content::annotation::Comment;
use loki_doc_model::content::block::Block;
use loki_doc_model::content::inline::Inline;

use crate::docx::reader::util::{attr_val, local_name};
use crate::error::{OoxmlError, OoxmlResult};

/// Parses `word/comments.xml` into the document's comments.
///
/// Each `w:p` in a comment becomes a [`Block::Para`]; run text is concatenated
/// as plain text (inline formatting inside comments is not preserved).
pub(crate) fn parse_comments(xml: &[u8]) -> OoxmlResult<Vec<Comment>> {
    let mut reader = Reader::from_reader(xml);
    reader.config_mut().trim_text(false);
    let mut buf = Vec::new();
    let mut comments = Vec::new();

    // State for the comment currently open.
    let mut current: Option<Comment> = None;
    let mut para_text = String::new();
    let mut in_text = false;
    let mut in_paragraph = false;

    loop {
        match reader.read_event_into(&mut buf) {
            Ok(Event::Start(ref e)) => match local_name(e.local_name().as_ref()) {
                b"comment" => {
                    let mut c = Comment::new(attr_val(e, b"id").unwrap_or_default());
                    c.author = attr_val(e, b"author");
                    c.date = attr_val(e, b"date")
                        .and_then(|d| DateTime::parse_from_rfc3339(&d).ok())
                        .map(|d| d.with_timezone(&chrono::Utc));
                    current = Some(c);
                }
                b"p" if current.is_some() => {
                    in_paragraph = true;
                    para_text.clear();
                }
                b"t" if current.is_some() => in_text = true,
                _ => {}
            },
            Ok(Event::Text(ref t)) if in_text => {
                if let Ok(s) = t.unescape() {
                    para_text.push_str(&s);
                }
            }
            Ok(Event::End(ref e)) => match local_name(e.local_name().as_ref()) {
                b"t" => in_text = false,
                b"p" if in_paragraph => {
                    in_paragraph = false;
                    if let Some(c) = current.as_mut() {
                        c.body.push(Block::Para(vec![Inline::Str(std::mem::take(
                            &mut para_text,
                        ))]));
                    }
                }
                b"comment" => {
                    if let Some(c) = current.take() {
                        comments.push(c);
                    }
                }
                _ => {}
            },
            Ok(Event::Eof) => break,
            Err(e) => {
                return Err(OoxmlError::Xml {
                    part: "word/comments.xml".into(),
                    source: e,
                });
            }
            _ => {}
        }
        buf.clear();
    }
    Ok(comments)
}
