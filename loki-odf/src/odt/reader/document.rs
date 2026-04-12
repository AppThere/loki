// Copyright 2024-2026 AppThere
//
// Licensed under the Apache License, Version 2.0 (the "License");
// you may not use this file except in compliance with the License.
// You may obtain a copy of the License at
//
//     http://www.apache.org/licenses/LICENSE-2.0
//
// Unless required by applicable law or agreed to in writing, software
// distributed under the License is distributed on an "AS IS" BASIS,
// WITHOUT WARRANTIES OR CONDITIONS OF ANY KIND, either express or implied.
// See the License for the specific language governing permissions and
// limitations under the License.

//! Reader for `content.xml` — paragraph-level and inline-level parsing.
//!
//! # Caller contract
//!
//! Every `read_X(reader, tag)` function is called **after** its opening
//! `Start` event has been consumed. It reads until — and including — the
//! matching `End` event at the same nesting depth. Body-level parsing
//! (`read_document`, `read_list`, `read_table`, etc.) is implemented in a
//! later session.
// Functions are not yet called from outside this module; suppress lint.
#![allow(dead_code)]
// `drop(ref_binding)` is a deliberate NLL-boundary hint that has no runtime
// effect; silence the suggestion to use `let _ = …` instead.
#![allow(dropping_references)]

use quick_xml::events::{BytesStart, Event};
use quick_xml::Reader;

use crate::error::{OdfError, OdfResult};
use crate::odt::model::fields::OdfField;
use crate::odt::model::frames::{OdfFrame, OdfFrameKind};
use crate::odt::model::notes::{OdfNote, OdfNoteClass};
use crate::odt::model::paragraph::{
    OdfHyperlink, OdfParagraph, OdfParagraphChild, OdfSpan,
};
use crate::xml_util::local_attr_val;

// ── Utilities ─────────────────────────────────────────────────────────────────

/// Skip all events until the end of the current element.
///
/// Must be called immediately after consuming the `Start` event for the
/// element to skip. Tracks nesting depth so that child elements with the
/// same local name are handled correctly. On return the matching `End`
/// event has been consumed.
pub(crate) fn skip_element(reader: &mut Reader<&[u8]>) -> OdfResult<()> {
    let mut buf = Vec::new();
    let mut depth: u32 = 1;
    loop {
        buf.clear();
        match reader.read_event_into(&mut buf) {
            Ok(Event::Start(_)) => depth += 1,
            Ok(Event::End(_)) => {
                depth -= 1;
                if depth == 0 {
                    return Ok(());
                }
            }
            Ok(Event::Eof) => return Ok(()),
            Err(e) => {
                return Err(OdfError::Xml {
                    part: "content.xml".to_string(),
                    source: e,
                })
            }
            _ => {}
        }
    }
}

/// Collect all text-node content inside the current element.
///
/// Must be called immediately after consuming the `Start` event. Only
/// top-level (depth-1) text nodes are collected; text inside child elements
/// is silently skipped. On return the matching `End` event has been consumed.
pub(crate) fn read_text_content(reader: &mut Reader<&[u8]>) -> OdfResult<String> {
    let mut buf = Vec::new();
    let mut depth: u32 = 1;
    let mut text = String::new();
    loop {
        buf.clear();
        match reader.read_event_into(&mut buf) {
            Ok(Event::Start(_)) => depth += 1,
            Ok(Event::End(_)) => {
                depth -= 1;
                if depth == 0 {
                    return Ok(text);
                }
            }
            Ok(Event::Text(ref t)) if depth == 1 => {
                let s = t.unescape().map_err(|e| OdfError::Xml {
                    part: "content.xml".to_string(),
                    source: quick_xml::Error::from(e),
                })?;
                text.push_str(&s);
            }
            Ok(Event::Eof) => return Ok(text),
            Err(e) => {
                return Err(OdfError::Xml {
                    part: "content.xml".to_string(),
                    source: e,
                })
            }
            _ => {}
        }
    }
}

// ── Paragraph ─────────────────────────────────────────────────────────────────

/// Parse a `text:p` or `text:h` element.
///
/// Called after consuming the `Start` event. `tag` carries the element
/// attributes (`text:style-name`, `text:outline-level`). On return the
/// matching `End` event has been consumed. ODF 1.3 §5.1.
pub(crate) fn read_paragraph(
    reader: &mut Reader<&[u8]>,
    tag: &BytesStart<'_>,
) -> OdfResult<OdfParagraph> {
    let local = tag.local_name().into_inner();
    let is_heading = local == b"h";
    let style_name = local_attr_val(tag, b"style-name");
    let outline_level: Option<u8> = local_attr_val(tag, b"outline-level")
        .and_then(|s| s.parse().ok());

    let children = read_inline_children(reader)?;

    Ok(OdfParagraph {
        style_name,
        outline_level,
        is_heading,
        children,
        list_context: None,
    })
}

// ── Inline children ───────────────────────────────────────────────────────────

/// Collect all inline children until the first `End` event at depth 0.
///
/// Called immediately after consuming the `Start` event of the containing
/// element (paragraph, span, or hyperlink). Returns when the first `End`
/// event is encountered — which, for well-formed XML and given that all
/// recognised child `Start` events are fully consumed (including their
/// matching `End`), is always the `End` of the containing element.
///
/// Unrecognised inline elements are skipped and represented as
/// [`OdfParagraphChild::Other`].
fn read_inline_children(
    reader: &mut Reader<&[u8]>,
) -> OdfResult<Vec<OdfParagraphChild>> {
    let mut children = Vec::new();
    let mut buf = Vec::new();
    loop {
        buf.clear();
        match reader.read_event_into(&mut buf) {
            // ── Start elements ─────────────────────────────────────────────
            Ok(Event::Start(ref e)) => {
                let local = e.local_name().into_inner().to_vec();
                match local.as_slice() {
                    // Styled span — ODF 1.3 §6.1
                    b"span" => {
                        let style_name = local_attr_val(e, b"style-name");
                        drop(e);
                        let span_children = read_inline_children(reader)?;
                        children.push(OdfParagraphChild::Span(OdfSpan {
                            style_name,
                            children: span_children,
                        }));
                    }
                    // Hyperlink — ODF 1.3 §6.4
                    b"a" => {
                        let href = local_attr_val(e, b"href");
                        let style_name = local_attr_val(e, b"style-name");
                        drop(e);
                        let link_children = read_inline_children(reader)?;
                        children.push(OdfParagraphChild::Hyperlink(OdfHyperlink {
                            href,
                            style_name,
                            children: link_children,
                        }));
                    }
                    // Footnote / endnote — ODF 1.3 §6.3
                    b"note" => {
                        let id = local_attr_val(e, b"id");
                        let note_class =
                            match local_attr_val(e, b"note-class").as_deref() {
                                Some("endnote") => OdfNoteClass::Endnote,
                                _ => OdfNoteClass::Footnote,
                            };
                        drop(e);
                        let note = read_note_body(reader, id, note_class)?;
                        children.push(OdfParagraphChild::Note(note));
                    }
                    // Drawing frame — ODF 1.3 §10.4
                    b"frame" => {
                        let name = local_attr_val(e, b"name");
                        let style_name = local_attr_val(e, b"style-name");
                        let anchor_type = local_attr_val(e, b"anchor-type");
                        let width = local_attr_val(e, b"width");
                        let height = local_attr_val(e, b"height");
                        let x = local_attr_val(e, b"x");
                        let y = local_attr_val(e, b"y");
                        drop(e);
                        let kind = read_frame_kind(reader)?;
                        children.push(OdfParagraphChild::Frame(OdfFrame {
                            name,
                            style_name,
                            anchor_type,
                            width,
                            height,
                            x,
                            y,
                            kind,
                        }));
                    }
                    // Bookmark (point or range-start) — ODF 1.3 §6.6
                    b"bookmark" | b"bookmark-start" => {
                        let id = local_attr_val(e, b"id");
                        let name =
                            local_attr_val(e, b"name").unwrap_or_default();
                        drop(e);
                        skip_element(reader)?;
                        children.push(OdfParagraphChild::Bookmark { id, name });
                    }
                    // Bookmark range-end — ODF 1.3 §6.6
                    b"bookmark-end" => {
                        let name =
                            local_attr_val(e, b"name").unwrap_or_default();
                        drop(e);
                        skip_element(reader)?;
                        children.push(OdfParagraphChild::BookmarkEnd { name });
                    }
                    // ── Text fields ────────────────────────────────────────
                    b"page-number" => {
                        let select_page = local_attr_val(e, b"select-page");
                        drop(e);
                        skip_element(reader)?;
                        children.push(OdfParagraphChild::Field(
                            OdfField::PageNumber { select_page },
                        ));
                    }
                    b"page-count" => {
                        drop(e);
                        skip_element(reader)?;
                        children.push(OdfParagraphChild::Field(OdfField::PageCount));
                    }
                    b"date" => {
                        let data_style = local_attr_val(e, b"data-style-name");
                        let fixed_value = local_attr_val(e, b"date-value");
                        drop(e);
                        skip_element(reader)?;
                        children.push(OdfParagraphChild::Field(OdfField::Date {
                            data_style,
                            fixed_value,
                        }));
                    }
                    b"time" => {
                        let data_style = local_attr_val(e, b"data-style-name");
                        let fixed_value = local_attr_val(e, b"time-value");
                        drop(e);
                        skip_element(reader)?;
                        children.push(OdfParagraphChild::Field(OdfField::Time {
                            data_style,
                            fixed_value,
                        }));
                    }
                    b"title" => {
                        drop(e);
                        skip_element(reader)?;
                        children.push(OdfParagraphChild::Field(OdfField::Title));
                    }
                    b"subject" => {
                        drop(e);
                        skip_element(reader)?;
                        children.push(OdfParagraphChild::Field(OdfField::Subject));
                    }
                    b"author-name" => {
                        drop(e);
                        skip_element(reader)?;
                        children.push(OdfParagraphChild::Field(OdfField::AuthorName));
                    }
                    b"file-name" => {
                        let display = local_attr_val(e, b"display");
                        drop(e);
                        skip_element(reader)?;
                        children.push(OdfParagraphChild::Field(
                            OdfField::FileName { display },
                        ));
                    }
                    b"word-count" => {
                        drop(e);
                        skip_element(reader)?;
                        children.push(OdfParagraphChild::Field(OdfField::WordCount));
                    }
                    b"chapter" => {
                        let display_levels: u8 =
                            local_attr_val(e, b"display-levels")
                                .and_then(|s| s.parse().ok())
                                .unwrap_or(1);
                        drop(e);
                        skip_element(reader)?;
                        children.push(OdfParagraphChild::Field(
                            OdfField::ChapterName { display_levels },
                        ));
                    }
                    b"bookmark-ref" | b"sequence-ref" => {
                        let ref_name =
                            local_attr_val(e, b"ref-name").unwrap_or_default();
                        let display = local_attr_val(e, b"reference-format")
                            .or_else(|| local_attr_val(e, b"display"));
                        drop(e);
                        skip_element(reader)?;
                        children.push(OdfParagraphChild::Field(
                            OdfField::CrossReference { ref_name, display },
                        ));
                    }
                    // Any other inline element: skip and record as Other
                    _ => {
                        drop(e);
                        skip_element(reader)?;
                        children.push(OdfParagraphChild::Other);
                    }
                }
            }
            // ── Empty elements ─────────────────────────────────────────────
            Ok(Event::Empty(ref e)) => {
                let local = e.local_name().into_inner();
                match local {
                    b"tab" => children.push(OdfParagraphChild::Tab),
                    b"line-break" => children.push(OdfParagraphChild::LineBreak),
                    b"soft-page-break" => {
                        children.push(OdfParagraphChild::SoftReturn)
                    }
                    b"s" => {
                        let count: u32 = local_attr_val(e, b"c")
                            .and_then(|s| s.parse().ok())
                            .unwrap_or(1);
                        children.push(OdfParagraphChild::Space { count });
                    }
                    b"bookmark" | b"bookmark-start" => {
                        let id = local_attr_val(e, b"id");
                        let name =
                            local_attr_val(e, b"name").unwrap_or_default();
                        children.push(OdfParagraphChild::Bookmark { id, name });
                    }
                    b"bookmark-end" => {
                        let name =
                            local_attr_val(e, b"name").unwrap_or_default();
                        children.push(OdfParagraphChild::BookmarkEnd { name });
                    }
                    b"page-number" => {
                        let select_page = local_attr_val(e, b"select-page");
                        children.push(OdfParagraphChild::Field(
                            OdfField::PageNumber { select_page },
                        ));
                    }
                    b"page-count" => {
                        children.push(OdfParagraphChild::Field(OdfField::PageCount));
                    }
                    b"date" => {
                        let data_style = local_attr_val(e, b"data-style-name");
                        let fixed_value = local_attr_val(e, b"date-value");
                        children.push(OdfParagraphChild::Field(OdfField::Date {
                            data_style,
                            fixed_value,
                        }));
                    }
                    b"time" => {
                        let data_style = local_attr_val(e, b"data-style-name");
                        let fixed_value = local_attr_val(e, b"time-value");
                        children.push(OdfParagraphChild::Field(OdfField::Time {
                            data_style,
                            fixed_value,
                        }));
                    }
                    b"title" => {
                        children.push(OdfParagraphChild::Field(OdfField::Title));
                    }
                    b"subject" => {
                        children.push(OdfParagraphChild::Field(OdfField::Subject));
                    }
                    b"author-name" => {
                        children.push(OdfParagraphChild::Field(OdfField::AuthorName));
                    }
                    b"file-name" => {
                        let display = local_attr_val(e, b"display");
                        children.push(OdfParagraphChild::Field(
                            OdfField::FileName { display },
                        ));
                    }
                    b"word-count" => {
                        children.push(OdfParagraphChild::Field(OdfField::WordCount));
                    }
                    b"chapter" => {
                        let display_levels: u8 =
                            local_attr_val(e, b"display-levels")
                                .and_then(|s| s.parse().ok())
                                .unwrap_or(1);
                        children.push(OdfParagraphChild::Field(
                            OdfField::ChapterName { display_levels },
                        ));
                    }
                    _ => children.push(OdfParagraphChild::Other),
                }
            }
            // ── Text nodes ─────────────────────────────────────────────────
            Ok(Event::Text(ref t)) => {
                let s = t.unescape().map_err(|e| OdfError::Xml {
                    part: "content.xml".to_string(),
                    source: quick_xml::Error::from(e),
                })?;
                if !s.is_empty() {
                    children.push(OdfParagraphChild::Text(s.into_owned()));
                }
            }
            // ── End: the containing element has closed ──────────────────────
            Ok(Event::End(_)) => break,
            Ok(Event::Eof) => break,
            Err(e) => {
                return Err(OdfError::Xml {
                    part: "content.xml".to_string(),
                    source: e,
                })
            }
            _ => {}
        }
    }
    Ok(children)
}

// ── Note ──────────────────────────────────────────────────────────────────────

/// Parse the body of a `text:note` element.
///
/// Called after the `Start` event for `text:note` has been consumed and
/// `id` / `note_class` have been extracted from its attributes. Reads until
/// the matching `</text:note>` end tag. ODF 1.3 §6.3.
fn read_note_body(
    reader: &mut Reader<&[u8]>,
    id: Option<String>,
    note_class: OdfNoteClass,
) -> OdfResult<OdfNote> {
    let mut citation: Option<String> = None;
    let mut body: Vec<OdfParagraph> = Vec::new();
    let mut buf = Vec::new();

    loop {
        buf.clear();
        match reader.read_event_into(&mut buf) {
            Ok(Event::Start(ref e)) => {
                let local = e.local_name().into_inner().to_vec();
                match local.as_slice() {
                    b"note-citation" => {
                        drop(e);
                        citation = Some(read_text_content(reader)?);
                    }
                    b"note-body" => {
                        drop(e); // children parsed by subsequent iterations
                    }
                    b"p" | b"h" => {
                        let para = read_paragraph(reader, e)?;
                        body.push(para);
                    }
                    _ => {
                        drop(e);
                        skip_element(reader)?;
                    }
                }
            }
            Ok(Event::End(ref e)) => {
                if e.local_name().into_inner() == b"note" {
                    break;
                }
            }
            Ok(Event::Eof) => break,
            Err(e) => {
                return Err(OdfError::Xml {
                    part: "content.xml".to_string(),
                    source: e,
                })
            }
            _ => {}
        }
    }

    Ok(OdfNote {
        id,
        note_class,
        citation,
        body,
    })
}

// ── Frame ─────────────────────────────────────────────────────────────────────

/// Parse a `draw:frame` element.
///
/// Called after consuming the `Start` event. `tag` carries the frame
/// geometry and style attributes. On return the matching `End` event
/// has been consumed. ODF 1.3 §10.4.
pub(crate) fn read_frame(
    reader: &mut Reader<&[u8]>,
    tag: &BytesStart<'_>,
) -> OdfResult<OdfFrame> {
    let name = local_attr_val(tag, b"name");
    let style_name = local_attr_val(tag, b"style-name");
    let anchor_type = local_attr_val(tag, b"anchor-type");
    let width = local_attr_val(tag, b"width");
    let height = local_attr_val(tag, b"height");
    let x = local_attr_val(tag, b"x");
    let y = local_attr_val(tag, b"y");
    let kind = read_frame_kind(reader)?;
    Ok(OdfFrame {
        name,
        style_name,
        anchor_type,
        width,
        height,
        x,
        y,
        kind,
    })
}

/// Determine the [`OdfFrameKind`] by reading the first recognised child of
/// a `draw:frame` element.
///
/// Called after frame attributes have been extracted. Reads until
/// `</draw:frame>`. ODF 1.3 §10.4–§10.7.
fn read_frame_kind(reader: &mut Reader<&[u8]>) -> OdfResult<OdfFrameKind> {
    let mut kind = OdfFrameKind::Other;
    let mut buf = Vec::new();
    loop {
        buf.clear();
        match reader.read_event_into(&mut buf) {
            Ok(Event::Start(ref e)) => {
                let local = e.local_name().into_inner().to_vec();
                match local.as_slice() {
                    b"image" => {
                        let href =
                            local_attr_val(e, b"href").unwrap_or_default();
                        let media_type = local_attr_val(e, b"type");
                        drop(e);
                        let (title, desc) = read_image_children(reader)?;
                        kind =
                            OdfFrameKind::Image { href, media_type, title, desc };
                    }
                    b"text-box" => {
                        drop(e);
                        let paragraphs = read_text_box_paragraphs(reader)?;
                        kind = OdfFrameKind::TextBox { paragraphs };
                    }
                    _ => {
                        drop(e);
                        skip_element(reader)?;
                    }
                }
            }
            Ok(Event::Empty(ref e)) => {
                if e.local_name().into_inner() == b"image" {
                    let href =
                        local_attr_val(e, b"href").unwrap_or_default();
                    let media_type = local_attr_val(e, b"type");
                    kind = OdfFrameKind::Image {
                        href,
                        media_type,
                        title: None,
                        desc: None,
                    };
                }
            }
            Ok(Event::End(_)) => break,
            Ok(Event::Eof) => break,
            Err(e) => {
                return Err(OdfError::Xml {
                    part: "content.xml".to_string(),
                    source: e,
                })
            }
            _ => {}
        }
    }
    Ok(kind)
}

/// Read `svg:title` and `svg:desc` children of a `draw:image` element.
///
/// Called after consuming `Start(image)`. Returns `(title, desc)` and
/// positions the reader after `</draw:image>`. ODF 1.3 §10.5.
fn read_image_children(
    reader: &mut Reader<&[u8]>,
) -> OdfResult<(Option<String>, Option<String>)> {
    let mut title: Option<String> = None;
    let mut desc: Option<String> = None;
    let mut buf = Vec::new();
    loop {
        buf.clear();
        match reader.read_event_into(&mut buf) {
            Ok(Event::Start(ref e)) => {
                let local = e.local_name().into_inner().to_vec();
                drop(e);
                match local.as_slice() {
                    b"title" => title = Some(read_text_content(reader)?),
                    b"desc" => desc = Some(read_text_content(reader)?),
                    _ => skip_element(reader)?,
                }
            }
            Ok(Event::End(_)) => break,
            Ok(Event::Eof) => break,
            Err(e) => {
                return Err(OdfError::Xml {
                    part: "content.xml".to_string(),
                    source: e,
                })
            }
            _ => {}
        }
    }
    Ok((title, desc))
}

/// Read `text:p` / `text:h` children of a `draw:text-box` element.
///
/// Called after consuming `Start(text-box)`. Returns the paragraphs and
/// positions the reader after `</draw:text-box>`. ODF 1.3 §10.7.
fn read_text_box_paragraphs(
    reader: &mut Reader<&[u8]>,
) -> OdfResult<Vec<OdfParagraph>> {
    let mut paragraphs = Vec::new();
    let mut buf = Vec::new();
    loop {
        buf.clear();
        match reader.read_event_into(&mut buf) {
            Ok(Event::Start(ref e)) => {
                let local = e.local_name().into_inner();
                if local == b"p" || local == b"h" {
                    let para = read_paragraph(reader, e)?;
                    paragraphs.push(para);
                } else {
                    drop(e);
                    skip_element(reader)?;
                }
            }
            Ok(Event::End(_)) => break,
            Ok(Event::Eof) => break,
            Err(e) => {
                return Err(OdfError::Xml {
                    part: "content.xml".to_string(),
                    source: e,
                })
            }
            _ => {}
        }
    }
    Ok(paragraphs)
}

// ── Tests ──────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use quick_xml::events::Event;
    use quick_xml::Reader;

    use crate::odt::model::frames::OdfFrameKind;
    use crate::odt::model::notes::OdfNoteClass;

    // ── Test helper ───────────────────────────────────────────────────────────

    /// Parse the first `text:p` or `text:h` found in `xml` and return the
    /// resulting [`OdfParagraph`].
    fn parse_first_para(xml: &[u8]) -> OdfParagraph {
        let mut reader = Reader::from_reader(xml);
        reader.config_mut().trim_text(false);
        let mut buf = Vec::new();
        loop {
            buf.clear();
            match reader.read_event_into(&mut buf).expect("xml error") {
                Event::Start(ref e) => {
                    let local = e.local_name().into_inner();
                    if local == b"p" || local == b"h" {
                        return read_paragraph(&mut reader, e)
                            .expect("read_paragraph failed");
                    }
                }
                Event::Eof => panic!("no text:p / text:h found in test XML"),
                _ => {}
            }
        }
    }

    // ── Test cases ────────────────────────────────────────────────────────────

    #[test]
    fn plain_text_paragraph() {
        let xml = br#"<?xml version="1.0"?>
<root xmlns:text="urn:oasis:names:tc:opendocument:xmlns:text:1.0">
  <text:p text:style-name="Body_20_Text">Hello, World!</text:p>
</root>"#;
        let para = parse_first_para(xml);
        assert_eq!(para.style_name.as_deref(), Some("Body_20_Text"));
        assert!(!para.is_heading);
        assert_eq!(para.outline_level, None);
        assert_eq!(para.children.len(), 1);
        match &para.children[0] {
            OdfParagraphChild::Text(s) => assert_eq!(s, "Hello, World!"),
            other => panic!("expected Text, got {:?}", other),
        }
    }

    #[test]
    fn paragraph_with_span() {
        let xml = br#"<?xml version="1.0"?>
<root xmlns:text="urn:oasis:names:tc:opendocument:xmlns:text:1.0">
  <text:p>Hello <text:span text:style-name="Bold">World</text:span>!</text:p>
</root>"#;
        let para = parse_first_para(xml);
        assert_eq!(para.children.len(), 3);
        match &para.children[1] {
            OdfParagraphChild::Span(span) => {
                assert_eq!(span.style_name.as_deref(), Some("Bold"));
                assert_eq!(span.children.len(), 1);
                match &span.children[0] {
                    OdfParagraphChild::Text(s) => assert_eq!(s, "World"),
                    other => panic!("expected Text in span, got {:?}", other),
                }
            }
            other => panic!("expected Span, got {:?}", other),
        }
    }

    #[test]
    fn heading_with_outline_level() {
        let xml = br#"<?xml version="1.0"?>
<root xmlns:text="urn:oasis:names:tc:opendocument:xmlns:text:1.0">
  <text:h text:style-name="Heading_20_2" text:outline-level="2">Section 1.1</text:h>
</root>"#;
        let para = parse_first_para(xml);
        assert!(para.is_heading);
        assert_eq!(para.outline_level, Some(2));
        assert_eq!(para.style_name.as_deref(), Some("Heading_20_2"));
        assert_eq!(para.children.len(), 1);
        match &para.children[0] {
            OdfParagraphChild::Text(s) => assert_eq!(s, "Section 1.1"),
            other => panic!("expected Text, got {:?}", other),
        }
    }

    #[test]
    fn paragraph_with_footnote() {
        let xml = br#"<?xml version="1.0"?>
<root xmlns:text="urn:oasis:names:tc:opendocument:xmlns:text:1.0">
  <text:p>See note<text:note text:id="ftn1" text:note-class="footnote"><text:note-citation>1</text:note-citation><text:note-body><text:p text:style-name="Footnote">Footnote text.</text:p></text:note-body></text:note>.</text:p>
</root>"#;
        let para = parse_first_para(xml);
        let note = para
            .children
            .iter()
            .find_map(|c| match c {
                OdfParagraphChild::Note(n) => Some(n),
                _ => None,
            })
            .expect("no Note child");
        assert_eq!(note.id.as_deref(), Some("ftn1"));
        assert_eq!(note.note_class, OdfNoteClass::Footnote);
        assert_eq!(note.citation.as_deref(), Some("1"));
        assert_eq!(note.body.len(), 1);
        assert_eq!(note.body[0].style_name.as_deref(), Some("Footnote"));
        match &note.body[0].children[0] {
            OdfParagraphChild::Text(s) => assert_eq!(s, "Footnote text."),
            other => panic!("expected Text in footnote body, got {:?}", other),
        }
    }

    #[test]
    fn page_number_field() {
        let xml = br#"<?xml version="1.0"?>
<root xmlns:text="urn:oasis:names:tc:opendocument:xmlns:text:1.0">
  <text:p><text:page-number text:select-page="current">1</text:page-number></text:p>
</root>"#;
        let para = parse_first_para(xml);
        assert_eq!(para.children.len(), 1);
        match &para.children[0] {
            OdfParagraphChild::Field(OdfField::PageNumber { select_page }) => {
                assert_eq!(select_page.as_deref(), Some("current"));
            }
            other => panic!("expected PageNumber field, got {:?}", other),
        }
    }

    #[test]
    fn paragraph_with_hyperlink() {
        let xml = br#"<?xml version="1.0"?>
<root xmlns:text="urn:oasis:names:tc:opendocument:xmlns:text:1.0"
      xmlns:xlink="http://www.w3.org/1999/xlink">
  <text:p><text:a xlink:href="https://example.com" text:style-name="Internet_20_Link">Click here</text:a></text:p>
</root>"#;
        let para = parse_first_para(xml);
        assert_eq!(para.children.len(), 1);
        match &para.children[0] {
            OdfParagraphChild::Hyperlink(link) => {
                assert_eq!(link.href.as_deref(), Some("https://example.com"));
                assert_eq!(
                    link.style_name.as_deref(),
                    Some("Internet_20_Link")
                );
                assert_eq!(link.children.len(), 1);
                match &link.children[0] {
                    OdfParagraphChild::Text(s) => {
                        assert_eq!(s, "Click here")
                    }
                    other => {
                        panic!("expected Text in link, got {:?}", other)
                    }
                }
            }
            other => panic!("expected Hyperlink, got {:?}", other),
        }
    }

    #[test]
    fn paragraph_with_inline_image() {
        let xml = br#"<?xml version="1.0"?>
<root xmlns:text="urn:oasis:names:tc:opendocument:xmlns:text:1.0"
      xmlns:draw="urn:oasis:names:tc:opendocument:xmlns:drawing:1.0"
      xmlns:xlink="http://www.w3.org/1999/xlink"
      xmlns:svg="urn:oasis:names:tc:opendocument:xmlns:svg-compatible:1.0">
  <text:p>
    <draw:frame draw:name="Image1" text:anchor-type="as-char" svg:width="5cm" svg:height="3cm">
      <draw:image xlink:href="Pictures/img.png">
        <svg:title>Alt text</svg:title>
      </draw:image>
    </draw:frame>
  </text:p>
</root>"#;
        let para = parse_first_para(xml);
        let frame = para
            .children
            .iter()
            .find_map(|c| match c {
                OdfParagraphChild::Frame(f) => Some(f),
                _ => None,
            })
            .expect("no Frame child");
        assert_eq!(frame.name.as_deref(), Some("Image1"));
        assert_eq!(frame.anchor_type.as_deref(), Some("as-char"));
        assert_eq!(frame.width.as_deref(), Some("5cm"));
        assert_eq!(frame.height.as_deref(), Some("3cm"));
        match &frame.kind {
            OdfFrameKind::Image { href, title, .. } => {
                assert_eq!(href, "Pictures/img.png");
                assert_eq!(title.as_deref(), Some("Alt text"));
            }
            other => panic!("expected Image kind, got {:?}", other),
        }
    }

    #[test]
    fn space_and_tab_elements() {
        let xml = br#"<?xml version="1.0"?>
<root xmlns:text="urn:oasis:names:tc:opendocument:xmlns:text:1.0">
  <text:p>A<text:s text:c="3"/>B<text:tab/>C</text:p>
</root>"#;
        let para = parse_first_para(xml);
        assert_eq!(para.children.len(), 5);
        assert!(
            matches!(&para.children[0], OdfParagraphChild::Text(s) if s == "A")
        );
        assert!(
            matches!(&para.children[1], OdfParagraphChild::Space { count: 3 })
        );
        assert!(
            matches!(&para.children[2], OdfParagraphChild::Text(s) if s == "B")
        );
        assert!(matches!(&para.children[3], OdfParagraphChild::Tab));
        assert!(
            matches!(&para.children[4], OdfParagraphChild::Text(s) if s == "C")
        );
    }

    #[test]
    fn nested_spans() {
        let xml = br#"<?xml version="1.0"?>
<root xmlns:text="urn:oasis:names:tc:opendocument:xmlns:text:1.0">
  <text:p><text:span text:style-name="Outer"><text:span text:style-name="Inner">deep</text:span></text:span></text:p>
</root>"#;
        let para = parse_first_para(xml);
        assert_eq!(para.children.len(), 1);
        let outer = match &para.children[0] {
            OdfParagraphChild::Span(s) => s,
            other => panic!("expected outer Span, got {:?}", other),
        };
        assert_eq!(outer.style_name.as_deref(), Some("Outer"));
        assert_eq!(outer.children.len(), 1);
        let inner = match &outer.children[0] {
            OdfParagraphChild::Span(s) => s,
            other => panic!("expected inner Span, got {:?}", other),
        };
        assert_eq!(inner.style_name.as_deref(), Some("Inner"));
        assert_eq!(inner.children.len(), 1);
        match &inner.children[0] {
            OdfParagraphChild::Text(t) => assert_eq!(t, "deep"),
            other => panic!("expected Text in inner span, got {:?}", other),
        }
    }
}
