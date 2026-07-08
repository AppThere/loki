// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 AppThere Loki contributors

//! Reader for `meta.xml` — extracts [`OdfMeta`] from the document metadata
//! part. ODF 1.3 §3.1 (`office:meta`).
// Called by the importer added in a later session; suppress premature lint.
#![allow(dead_code)]

use quick_xml::Reader;
use quick_xml::events::Event;

use crate::error::{OdfError, OdfResult};
use crate::odt::model::document::OdfMeta;
use crate::xml_util::{resolve_general_ref, unescape_text};

/// Parse `meta.xml` bytes and return the extracted [`OdfMeta`].
///
/// Extracts `dc:title`, `dc:creator`, `dc:description`,
/// `meta:creation-date` → `created`, `dc:date` → `modified`, and
/// `meta:editing-cycles`.  Unknown elements are silently skipped.
pub(crate) fn read_meta(xml: &[u8]) -> OdfResult<OdfMeta> {
    let mut reader = Reader::from_reader(xml);
    reader.config_mut().trim_text(false);

    let mut buf = Vec::new();
    let mut meta = OdfMeta::default();

    // Local name of the element currently being collected, if any.
    let mut collecting: Option<Vec<u8>> = None;
    let mut collect_text = String::new();
    // The `meta:name` of the `meta:user-defined` element currently open.
    let mut user_name: Option<String> = None;

    loop {
        buf.clear();
        match reader.read_event_into(&mut buf) {
            Ok(Event::Start(ref e)) => {
                let local = e.local_name().into_inner().to_vec();
                match local.as_slice() {
                    b"title" | b"creator" | b"description" | b"creation-date" | b"date"
                    | b"editing-cycles" | b"initial-creator" | b"subject" | b"keyword" => {
                        collecting = Some(local);
                        collect_text.clear();
                    }
                    b"user-defined" => {
                        user_name = crate::xml_util::local_attr_val(e, b"name");
                        collecting = Some(local);
                        collect_text.clear();
                    }
                    _ => {
                        // Descending into an unrecognised element; stop
                        // collecting so we don't mix text from nested content.
                        collecting = None;
                    }
                }
            }
            Ok(Event::End(ref e)) => {
                let local = e.local_name().into_inner();
                if let Some(ref tag) = collecting
                    && tag.as_slice() == local
                {
                    let text = std::mem::take(&mut collect_text);
                    match tag.as_slice() {
                        b"title" => meta.title = Some(text),
                        b"creator" => meta.creator = Some(text),
                        b"description" => meta.description = Some(text),
                        b"creation-date" => meta.created = Some(text),
                        b"date" => meta.modified = Some(text),
                        b"editing-cycles" => {
                            meta.editing_cycles = text.parse().ok();
                        }
                        b"initial-creator" => meta.initial_creator = Some(text),
                        b"subject" => meta.subject = Some(text),
                        b"keyword" => meta.keywords.push(text),
                        b"user-defined" => {
                            if let Some(name) = user_name.take() {
                                meta.user_defined.push((name, text));
                            }
                        }
                        _ => {}
                    }
                    collecting = None;
                }
            }
            Ok(Event::Text(ref t)) => {
                if collecting.is_some() {
                    let s = unescape_text(t).map_err(|e| OdfError::Xml {
                        part: "meta.xml".to_string(),
                        source: e,
                    })?;
                    collect_text.push_str(&s);
                }
            }
            Ok(Event::GeneralRef(ref r)) => {
                if collecting.is_some() {
                    let s = resolve_general_ref(r).map_err(|e| OdfError::Xml {
                        part: "meta.xml".to_string(),
                        source: e,
                    })?;
                    collect_text.push_str(&s);
                }
            }
            Ok(Event::Eof) => break,
            Err(e) => {
                return Err(OdfError::Xml {
                    part: "meta.xml".to_string(),
                    source: e,
                });
            }
            _ => {}
        }
    }

    Ok(meta)
}

// ── Tests ──────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn read_meta_title_and_creation_date() {
        let xml = br#"<?xml version="1.0"?>
<office:document-meta xmlns:office="urn:oasis:names:tc:opendocument:xmlns:office:1.0"
                      xmlns:dc="http://purl.org/dc/elements/1.1/"
                      xmlns:meta="urn:oasis:names:tc:opendocument:xmlns:meta:1.0">
  <office:meta>
    <dc:title>My Document</dc:title>
    <meta:creation-date>2024-01-15T10:00:00</meta:creation-date>
  </office:meta>
</office:document-meta>"#;

        let meta = read_meta(xml).unwrap();
        assert_eq!(meta.title.as_deref(), Some("My Document"));
        assert_eq!(meta.created.as_deref(), Some("2024-01-15T10:00:00"));
        assert!(meta.creator.is_none());
        assert!(meta.modified.is_none());
    }

    #[test]
    fn read_meta_all_fields() {
        let xml = br#"<?xml version="1.0"?>
<office:document-meta xmlns:office="urn:oasis:names:tc:opendocument:xmlns:office:1.0"
                      xmlns:dc="http://purl.org/dc/elements/1.1/"
                      xmlns:meta="urn:oasis:names:tc:opendocument:xmlns:meta:1.0">
  <office:meta>
    <dc:title>Test Doc</dc:title>
    <dc:subject>Test Subject</dc:subject>
    <dc:creator>Alice</dc:creator>
    <meta:initial-creator>Bob</meta:initial-creator>
    <dc:description>A test document</dc:description>
    <meta:creation-date>2023-06-01T09:00:00</meta:creation-date>
    <dc:date>2024-03-20T14:30:00</dc:date>
    <meta:editing-cycles>5</meta:editing-cycles>
    <meta:keyword>keyword1</meta:keyword>
    <meta:keyword>keyword2</meta:keyword>
  </office:meta>
</office:document-meta>"#;

        let meta = read_meta(xml).unwrap();
        assert_eq!(meta.title.as_deref(), Some("Test Doc"));
        assert_eq!(meta.subject.as_deref(), Some("Test Subject"));
        assert_eq!(meta.creator.as_deref(), Some("Alice"));
        assert_eq!(meta.initial_creator.as_deref(), Some("Bob"));
        assert_eq!(meta.description.as_deref(), Some("A test document"));
        assert_eq!(meta.created.as_deref(), Some("2023-06-01T09:00:00"));
        assert_eq!(meta.modified.as_deref(), Some("2024-03-20T14:30:00"));
        assert_eq!(meta.editing_cycles, Some(5));
        assert_eq!(
            meta.keywords,
            vec!["keyword1".to_string(), "keyword2".to_string()]
        );
    }

    #[test]
    fn read_meta_empty_returns_default() {
        let xml = br#"<?xml version="1.0"?>
<office:document-meta xmlns:office="urn:oasis:names:tc:opendocument:xmlns:office:1.0">
  <office:meta/>
</office:document-meta>"#;

        let meta = read_meta(xml).unwrap();
        assert!(meta.title.is_none());
        assert!(meta.creator.is_none());
        assert!(meta.editing_cycles.is_none());
    }
}
