// Copyright 2026 AppThere Loki contributors
// SPDX-License-Identifier: Apache-2.0

//! Reader for `word/footnotes.xml` and `word/endnotes.xml` → [`DocxNotes`].
//!
//! ECMA-376 §17.11 (footnotes and endnotes).

use quick_xml::events::Event;
use quick_xml::Reader;

use crate::docx::model::footnotes::{DocxNote, DocxNoteType, DocxNotes};
use crate::docx::reader::document::parse_paragraph;
use crate::docx::reader::util::{attr_val, local_name};
use crate::error::{OoxmlError, OoxmlResult};

/// Parses `word/footnotes.xml` or `word/endnotes.xml` into a [`DocxNotes`] model.
///
/// The `part` parameter is used only for error messages (e.g.
/// `"word/footnotes.xml"` or `"word/endnotes.xml"`).
///
/// ECMA-376 §17.11.12 (`w:footnotes`) / §17.11.2 (`w:endnotes`).
pub fn parse_notes(xml: &[u8], part: &str) -> OoxmlResult<DocxNotes> {
    let mut reader = Reader::from_reader(xml);
    reader.config_mut().trim_text(false);

    let mut result = DocxNotes::default();
    let mut buf = Vec::new();

    loop {
        match reader.read_event_into(&mut buf) {
            Ok(Event::Start(ref e)) => {
                match local_name(e.local_name().as_ref()) {
                    b"footnote" | b"endnote" => {
                        let id = attr_val(e, b"id")
                            .and_then(|v| v.parse::<i32>().ok())
                            .unwrap_or(0);
                        let note_type = attr_val(e, b"type")
                            .map(|t| match t.as_str() {
                                "separator" => DocxNoteType::Separator,
                                "continuationSeparator" => {
                                    DocxNoteType::ContinuationSeparator
                                }
                                _ => DocxNoteType::Normal,
                            })
                            .unwrap_or(DocxNoteType::Normal);
                        let note = parse_note(&mut reader, id, note_type, part)?;
                        result.notes.push(note);
                    }
                    _ => {}
                }
            }
            Ok(Event::Eof) => break,
            Err(e) => {
                return Err(OoxmlError::Xml {
                    part: part.to_owned(),
                    source: e,
                });
            }
            _ => {}
        }
        buf.clear();
    }
    Ok(result)
}

fn parse_note(
    reader: &mut Reader<&[u8]>,
    id: i32,
    note_type: DocxNoteType,
    part: &str,
) -> OoxmlResult<DocxNote> {
    let mut paragraphs = Vec::new();
    let mut buf = Vec::new();

    loop {
        match reader.read_event_into(&mut buf) {
            Ok(Event::Start(ref e)) if local_name(e.local_name().as_ref()) == b"p" => {
                let para = parse_paragraph(reader)?;
                paragraphs.push(para);
            }
            Ok(Event::End(ref e))
                if matches!(
                    local_name(e.local_name().as_ref()),
                    b"footnote" | b"endnote"
                ) =>
            {
                break;
            }
            Ok(Event::Eof) => break,
            Err(e) => {
                return Err(OoxmlError::Xml {
                    part: part.to_owned(),
                    source: e,
                });
            }
            _ => {}
        }
        buf.clear();
    }
    Ok(DocxNote {
        id,
        note_type,
        paragraphs,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    const MINIMAL_FOOTNOTES: &[u8] = br#"<?xml version="1.0" encoding="UTF-8"?>
<w:footnotes xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main">
  <w:footnote w:type="separator" w:id="-1">
    <w:p/>
  </w:footnote>
  <w:footnote w:id="1">
    <w:p>
      <w:r><w:t>Footnote text.</w:t></w:r>
    </w:p>
  </w:footnote>
</w:footnotes>"#;

    #[test]
    fn parses_two_footnotes() {
        let notes = parse_notes(MINIMAL_FOOTNOTES, "word/footnotes.xml").unwrap();
        assert_eq!(notes.notes.len(), 2);
    }

    #[test]
    fn separator_note_type() {
        let notes = parse_notes(MINIMAL_FOOTNOTES, "word/footnotes.xml").unwrap();
        let sep = notes.notes.iter().find(|n| n.id == -1).unwrap();
        assert_eq!(sep.note_type, DocxNoteType::Separator);
    }

    #[test]
    fn content_for_skips_separator() {
        let notes = parse_notes(MINIMAL_FOOTNOTES, "word/footnotes.xml").unwrap();
        // id=-1 is a separator → content_for returns None
        assert!(notes.content_for(-1).is_none());
        // id=1 is normal → returns Some
        assert!(notes.content_for(1).is_some());
    }
}
