// SPDX-License-Identifier: Apache-2.0
// Copyright (C) 2025 AppThere contributors

//! Parser for inline (paragraph-child) content.

use quick_xml::Reader;
use quick_xml::events::Event;

use crate::error::{OdfError, OdfResult};
use crate::odt::model::fields::OdfField;
use crate::odt::model::frames::OdfFrame;
use crate::odt::model::notes::OdfNoteClass;
use crate::odt::model::paragraph::{OdfHyperlink, OdfParagraphChild, OdfSpan};
use crate::xml_util::local_attr_val;

use super::frame::read_frame_kind;
use super::note::read_note_body;
use super::util::skip_element;

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
#[allow(clippy::too_many_lines)]
// Function body is a single large match over XML events; splitting would reduce readability.
pub(super) fn read_inline_children(
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
                        let note_class = match local_attr_val(e, b"note-class").as_deref() {
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
                        let name = local_attr_val(e, b"name").unwrap_or_default();
                        drop(e);
                        skip_element(reader)?;
                        children.push(OdfParagraphChild::Bookmark { id, name });
                    }
                    // Bookmark range-end — ODF 1.3 §6.6
                    b"bookmark-end" => {
                        let name = local_attr_val(e, b"name").unwrap_or_default();
                        drop(e);
                        skip_element(reader)?;
                        children.push(OdfParagraphChild::BookmarkEnd { name });
                    }
                    // ── Text fields ────────────────────────────────────────
                    b"page-number" => {
                        let select_page = local_attr_val(e, b"select-page");
                        drop(e);
                        skip_element(reader)?;
                        children.push(OdfParagraphChild::Field(OdfField::PageNumber {
                            select_page,
                        }));
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
                        children.push(OdfParagraphChild::Field(OdfField::FileName { display }));
                    }
                    b"word-count" => {
                        drop(e);
                        skip_element(reader)?;
                        children.push(OdfParagraphChild::Field(OdfField::WordCount));
                    }
                    b"chapter" => {
                        let display_levels: u8 = local_attr_val(e, b"display-levels")
                            .and_then(|s| s.parse().ok())
                            .unwrap_or(1);
                        drop(e);
                        skip_element(reader)?;
                        children.push(OdfParagraphChild::Field(OdfField::ChapterName {
                            display_levels,
                        }));
                    }
                    b"bookmark-ref" | b"sequence-ref" => {
                        let ref_name = local_attr_val(e, b"ref-name").unwrap_or_default();
                        let display = local_attr_val(e, b"reference-format")
                            .or_else(|| local_attr_val(e, b"display"));
                        drop(e);
                        skip_element(reader)?;
                        children.push(OdfParagraphChild::Field(OdfField::CrossReference {
                            ref_name,
                            display,
                        }));
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
                        children.push(OdfParagraphChild::SoftReturn);
                    }
                    b"s" => {
                        let count: u32 = local_attr_val(e, b"c")
                            .and_then(|s| s.parse().ok())
                            .unwrap_or(1);
                        children.push(OdfParagraphChild::Space { count });
                    }
                    b"bookmark" | b"bookmark-start" => {
                        let id = local_attr_val(e, b"id");
                        let name = local_attr_val(e, b"name").unwrap_or_default();
                        children.push(OdfParagraphChild::Bookmark { id, name });
                    }
                    b"bookmark-end" => {
                        let name = local_attr_val(e, b"name").unwrap_or_default();
                        children.push(OdfParagraphChild::BookmarkEnd { name });
                    }
                    b"page-number" => {
                        let select_page = local_attr_val(e, b"select-page");
                        children.push(OdfParagraphChild::Field(OdfField::PageNumber {
                            select_page,
                        }));
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
                        children.push(OdfParagraphChild::Field(OdfField::FileName { display }));
                    }
                    b"word-count" => {
                        children.push(OdfParagraphChild::Field(OdfField::WordCount));
                    }
                    b"chapter" => {
                        let display_levels: u8 = local_attr_val(e, b"display-levels")
                            .and_then(|s| s.parse().ok())
                            .unwrap_or(1);
                        children.push(OdfParagraphChild::Field(OdfField::ChapterName {
                            display_levels,
                        }));
                    }
                    _ => children.push(OdfParagraphChild::Other),
                }
            }
            // ── Text nodes ─────────────────────────────────────────────────
            Ok(Event::Text(ref t)) => {
                let s = t.unescape().map_err(|e| OdfError::Xml {
                    part: "content.xml".to_string(),
                    source: e,
                })?;
                if !s.is_empty() {
                    children.push(OdfParagraphChild::Text(s.into_owned()));
                }
            }
            // ── End: the containing element has closed ──────────────────────
            Ok(Event::End(_) | Event::Eof) => break,
            Err(e) => {
                return Err(OdfError::Xml {
                    part: "content.xml".to_string(),
                    source: e,
                });
            }
            _ => {}
        }
    }
    Ok(children)
}
