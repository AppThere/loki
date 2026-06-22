// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 AppThere Loki contributors

//! Reader for `word/comments.xml` (ECMA-376 §17.13.4.2): parses each
//! `w:comment` into a [`Comment`], with its author, date, and plain-text body.

use chrono::DateTime;
use quick_xml::Reader;
use quick_xml::events::Event;

use loki_doc_model::content::annotation::Comment;

use crate::docx::reader::util::{attr_val, local_name};
use crate::error::{OoxmlError, OoxmlResult};

/// Parses `word/comments.xml` into the document's comments.
///
/// The body is collected as plain text (paragraph runs joined, paragraphs
/// separated by `\n`) and stored in [`Comment::body_raw`] as UTF-8.
pub(crate) fn parse_comments(xml: &[u8]) -> OoxmlResult<Vec<Comment>> {
    let mut reader = Reader::from_reader(xml);
    reader.config_mut().trim_text(false);
    let mut buf = Vec::new();
    let mut comments = Vec::new();

    // State for the comment currently open.
    let mut current: Option<Comment> = None;
    let mut body = String::new();
    let mut in_text = false;
    let mut first_para = true;

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
                    body.clear();
                    first_para = true;
                }
                b"p" if current.is_some() => {
                    if !first_para {
                        body.push('\n');
                    }
                    first_para = false;
                }
                b"t" if current.is_some() => in_text = true,
                _ => {}
            },
            Ok(Event::Text(ref t)) if in_text => {
                if let Ok(s) = t.unescape() {
                    body.push_str(&s);
                }
            }
            Ok(Event::End(ref e)) => match local_name(e.local_name().as_ref()) {
                b"t" => in_text = false,
                b"comment" => {
                    if let Some(mut c) = current.take() {
                        c.body_raw = std::mem::take(&mut body).into_bytes();
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
