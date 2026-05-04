// Copyright 2026 AppThere Loki contributors
// SPDX-License-Identifier: Apache-2.0

//! Reader for `content.xml` — paragraph-level, inline-level, and body-level
//! parsing.
//!
//! # Caller contract
//!
//! Every `read_X(reader, tag)` function is called **after** its opening
//! `Start` event has been consumed. It reads until — and including — the
//! matching `End` event at the same nesting depth.
// Functions are not yet called from outside this module; suppress lint.
#![allow(dead_code)]
// `drop(ref_binding)` is a deliberate NLL-boundary hint that has no runtime
// effect; silence the suggestion to use `let _ = …` instead.
#![allow(dropping_references)]

use quick_xml::events::{BytesStart, Event};
use quick_xml::Reader;

use crate::error::{OdfError, OdfResult};
use crate::odt::model::document::{
    OdfBodyChild, OdfDocument, OdfList, OdfListItem, OdfListItemChild,
    OdfSection, OdfTableOfContent,
};
use crate::odt::model::fields::OdfField;
use crate::odt::model::frames::{OdfFrame, OdfFrameKind};
use crate::odt::model::notes::{OdfNote, OdfNoteClass};
use crate::odt::model::paragraph::{
    OdfHyperlink, OdfListContext, OdfParagraph, OdfParagraphChild, OdfSpan,
};
use crate::odt::model::tables::{
    OdfTable, OdfTableCell, OdfTableColDef, OdfTableRow,
};
use crate::version::OdfVersion;
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
                    source: e,
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
#[allow(clippy::too_many_lines)]
// Function body is a single large match over XML events; splitting would reduce readability.
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
            Ok(Event::End(_) | Event::Eof) => break,
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
            Ok(Event::End(_) | Event::Eof) => break,
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
            Ok(Event::End(_) | Event::Eof) => break,
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

// ── Table ─────────────────────────────────────────────────────────────────────

/// Parse a `table:table` element. ODF 1.3 §9.1.
///
/// Called after consuming the `Start` event for `table:table`.
pub(crate) fn read_table(
    reader: &mut Reader<&[u8]>,
    tag: &BytesStart<'_>,
) -> OdfResult<OdfTable> {
    let name = local_attr_val(tag, b"name");
    let style_name = local_attr_val(tag, b"style-name");
    let mut col_defs: Vec<OdfTableColDef> = Vec::new();
    let mut rows: Vec<OdfTableRow> = Vec::new();
    let mut buf = Vec::new();
    loop {
        buf.clear();
        match reader.read_event_into(&mut buf) {
            Ok(Event::Start(ref e)) => {
                let local = e.local_name().into_inner().to_vec();
                match local.as_slice() {
                    b"table-column" => {
                        let col_style = local_attr_val(e, b"style-name");
                        let columns_repeated: u32 =
                            local_attr_val(e, b"number-columns-repeated")
                                .and_then(|s| s.parse().ok())
                                .unwrap_or(1);
                        drop(e);
                        skip_element(reader)?;
                        col_defs.push(OdfTableColDef {
                            style_name: col_style,
                            columns_repeated,
                        });
                    }
                    b"table-header-rows" => {
                        drop(e);
                        read_table_header_rows(reader, &mut rows)?;
                    }
                    b"table-row" => {
                        let row = read_table_row(reader, e, false)?;
                        rows.push(row);
                    }
                    _ => {
                        drop(e);
                        skip_element(reader)?;
                    }
                }
            }
            Ok(Event::Empty(ref e)) => {
                let local = e.local_name().into_inner();
                if local == b"table-column" {
                    let col_style = local_attr_val(e, b"style-name");
                    let columns_repeated: u32 =
                        local_attr_val(e, b"number-columns-repeated")
                            .and_then(|s| s.parse().ok())
                            .unwrap_or(1);
                    col_defs.push(OdfTableColDef {
                        style_name: col_style,
                        columns_repeated,
                    });
                }
            }
            Ok(Event::End(_) | Event::Eof) => break,
            Err(e) => {
                return Err(OdfError::Xml {
                    part: "content.xml".to_string(),
                    source: e,
                })
            }
            _ => {}
        }
    }
    Ok(OdfTable { name, style_name, col_defs, rows })
}

/// Read rows inside `table:table-header-rows`, marking each as `is_header`.
fn read_table_header_rows(
    reader: &mut Reader<&[u8]>,
    rows: &mut Vec<OdfTableRow>,
) -> OdfResult<()> {
    let mut buf = Vec::new();
    loop {
        buf.clear();
        match reader.read_event_into(&mut buf) {
            Ok(Event::Start(ref e)) => {
                if e.local_name().into_inner() == b"table-row" {
                    let row = read_table_row(reader, e, true)?;
                    rows.push(row);
                } else {
                    drop(e);
                    skip_element(reader)?;
                }
            }
            Ok(Event::End(_) | Event::Eof) => break,
            Err(e) => {
                return Err(OdfError::Xml {
                    part: "content.xml".to_string(),
                    source: e,
                })
            }
            _ => {}
        }
    }
    Ok(())
}

/// Parse a `table:table-row` element. ODF 1.3 §9.3.
fn read_table_row(
    reader: &mut Reader<&[u8]>,
    tag: &BytesStart<'_>,
    _is_header: bool,
) -> OdfResult<OdfTableRow> {
    let style_name = local_attr_val(tag, b"style-name");
    let mut cells: Vec<OdfTableCell> = Vec::new();
    let mut buf = Vec::new();
    loop {
        buf.clear();
        match reader.read_event_into(&mut buf) {
            Ok(Event::Start(ref e)) => {
                let local = e.local_name().into_inner().to_vec();
                match local.as_slice() {
                    b"table-cell" => {
                        let cell = read_table_cell(reader, e, false)?;
                        cells.push(cell);
                    }
                    b"covered-table-cell" => {
                        drop(e);
                        skip_element(reader)?;
                        cells.push(OdfTableCell {
                            is_covered: true,
                            col_span: 1,
                            row_span: 1,
                            style_name: None,
                            value_type: None,
                            paragraphs: vec![],
                        });
                    }
                    _ => {
                        drop(e);
                        skip_element(reader)?;
                    }
                }
            }
            Ok(Event::Empty(ref e)) => {
                let local = e.local_name().into_inner();
                match local {
                    b"table-cell" => {
                        let style_name = local_attr_val(e, b"style-name");
                        let col_span: u32 =
                            local_attr_val(e, b"number-columns-spanned")
                                .and_then(|s| s.parse().ok())
                                .unwrap_or(1);
                        let row_span: u32 =
                            local_attr_val(e, b"number-rows-spanned")
                                .and_then(|s| s.parse().ok())
                                .unwrap_or(1);
                        let value_type = local_attr_val(e, b"value-type");
                        cells.push(OdfTableCell {
                            style_name,
                            col_span,
                            row_span,
                            is_covered: false,
                            value_type,
                            paragraphs: vec![],
                        });
                    }
                    b"covered-table-cell" => {
                        cells.push(OdfTableCell {
                            is_covered: true,
                            col_span: 1,
                            row_span: 1,
                            style_name: None,
                            value_type: None,
                            paragraphs: vec![],
                        });
                    }
                    _ => {}
                }
            }
            Ok(Event::End(_) | Event::Eof) => break,
            Err(e) => {
                return Err(OdfError::Xml {
                    part: "content.xml".to_string(),
                    source: e,
                })
            }
            _ => {}
        }
    }
    Ok(OdfTableRow { style_name, cells })
}

/// Parse a `table:table-cell` element. ODF 1.3 §9.4.
fn read_table_cell(
    reader: &mut Reader<&[u8]>,
    tag: &BytesStart<'_>,
    is_covered: bool,
) -> OdfResult<OdfTableCell> {
    let style_name = local_attr_val(tag, b"style-name");
    let col_span: u32 = local_attr_val(tag, b"number-columns-spanned")
        .and_then(|s| s.parse().ok())
        .unwrap_or(1);
    let row_span: u32 = local_attr_val(tag, b"number-rows-spanned")
        .and_then(|s| s.parse().ok())
        .unwrap_or(1);
    let value_type = local_attr_val(tag, b"value-type");
    let mut paragraphs: Vec<OdfParagraph> = Vec::new();
    let mut buf = Vec::new();
    loop {
        buf.clear();
        match reader.read_event_into(&mut buf) {
            Ok(Event::Start(ref e)) => {
                let local = e.local_name().into_inner().to_vec();
                match local.as_slice() {
                    b"p" | b"h" => {
                        let para = read_paragraph(reader, e)?;
                        paragraphs.push(para);
                    }
                    b"list" => {
                        let list = read_list(reader, e, None, 0)?;
                        collect_list_paragraphs(&list, &mut paragraphs);
                    }
                    _ => {
                        drop(e);
                        skip_element(reader)?;
                    }
                }
            }
            Ok(Event::End(_) | Event::Eof) => break,
            Err(e) => {
                return Err(OdfError::Xml {
                    part: "content.xml".to_string(),
                    source: e,
                })
            }
            _ => {}
        }
    }
    Ok(OdfTableCell { style_name, col_span, row_span, is_covered, value_type, paragraphs })
}

/// Recursively flatten all paragraphs out of a list into `out`.
fn collect_list_paragraphs(list: &OdfList, out: &mut Vec<OdfParagraph>) {
    for item in &list.items {
        for child in &item.children {
            match child {
                OdfListItemChild::Paragraph(p) | OdfListItemChild::Heading(p) => {
                    out.push(p.clone());
                }
                OdfListItemChild::List(nested) => {
                    collect_list_paragraphs(nested, out);
                }
            }
        }
    }
}

// ── List ──────────────────────────────────────────────────────────────────────

/// Parse a `text:list` element. ODF 1.3 §5.3.
///
/// Called after consuming the `Start` event. `parent_style` is the inherited
/// style from an enclosing list (used when this list has no explicit
/// `text:style-name`). `depth` is the 0-indexed nesting depth.
pub(crate) fn read_list(
    reader: &mut Reader<&[u8]>,
    tag: &BytesStart<'_>,
    parent_style: Option<&str>,
    depth: u8,
) -> OdfResult<OdfList> {
    let style_name = local_attr_val(tag, b"style-name");
    let xml_id = local_attr_val(tag, b"id");
    let continue_list = local_attr_val(tag, b"continue-list");
    let continue_numbering = local_attr_val(tag, b"continue-numbering")
        .is_some_and(|s| s == "true");

    let effective: Option<String> = style_name
        .clone()
        .or_else(|| parent_style.map(String::from));

    let mut items: Vec<OdfListItem> = Vec::new();
    let mut buf = Vec::new();
    loop {
        buf.clear();
        match reader.read_event_into(&mut buf) {
            Ok(Event::Start(ref e)) => {
                let local = e.local_name().into_inner().to_vec();
                match local.as_slice() {
                    b"list-item" | b"list-header" => {
                        let item = read_list_item(
                            reader,
                            e,
                            effective.as_deref(),
                            depth,
                        )?;
                        items.push(item);
                    }
                    _ => {
                        drop(e);
                        skip_element(reader)?;
                    }
                }
            }
            Ok(Event::End(_) | Event::Eof) => break,
            Err(e) => {
                return Err(OdfError::Xml {
                    part: "content.xml".to_string(),
                    source: e,
                })
            }
            _ => {}
        }
    }
    Ok(OdfList { xml_id, style_name, continue_list, continue_numbering, items })
}

/// Parse a `text:list-item` or `text:list-header` element. ODF 1.3 §5.3.
fn read_list_item(
    reader: &mut Reader<&[u8]>,
    tag: &BytesStart<'_>,
    list_style: Option<&str>,
    depth: u8,
) -> OdfResult<OdfListItem> {
    let start_value: Option<u32> = local_attr_val(tag, b"start-value")
        .and_then(|s| s.parse().ok());
    let mut children: Vec<OdfListItemChild> = Vec::new();
    let mut buf = Vec::new();
    loop {
        buf.clear();
        match reader.read_event_into(&mut buf) {
            Ok(Event::Start(ref e)) => {
                let local = e.local_name().into_inner().to_vec();
                match local.as_slice() {
                    b"p" => {
                        let mut para = read_paragraph(reader, e)?;
                        para.list_context = Some(OdfListContext {
                            style_name: list_style.map(String::from),
                            level: depth,
                            item_id: None,
                        });
                        children.push(OdfListItemChild::Paragraph(para));
                    }
                    b"h" => {
                        let mut para = read_paragraph(reader, e)?;
                        para.list_context = Some(OdfListContext {
                            style_name: list_style.map(String::from),
                            level: depth,
                            item_id: None,
                        });
                        children.push(OdfListItemChild::Heading(para));
                    }
                    b"list" => {
                        let nested = read_list(
                            reader,
                            e,
                            list_style,
                            depth.saturating_add(1),
                        )?;
                        children.push(OdfListItemChild::List(nested));
                    }
                    _ => {
                        drop(e);
                        skip_element(reader)?;
                    }
                }
            }
            Ok(Event::End(_) | Event::Eof) => break,
            Err(e) => {
                return Err(OdfError::Xml {
                    part: "content.xml".to_string(),
                    source: e,
                })
            }
            _ => {}
        }
    }
    Ok(OdfListItem { start_value, children })
}

// ── Table of contents ─────────────────────────────────────────────────────────

/// Parse a `text:table-of-content` element. ODF 1.3 §7.5.
pub(crate) fn read_toc(
    reader: &mut Reader<&[u8]>,
    tag: &BytesStart<'_>,
) -> OdfResult<OdfTableOfContent> {
    let name = local_attr_val(tag, b"name");
    let mut source_outline_level: u8 = 3;
    let mut body_paragraphs: Vec<OdfParagraph> = Vec::new();
    let mut buf = Vec::new();
    loop {
        buf.clear();
        match reader.read_event_into(&mut buf) {
            Ok(Event::Start(ref e)) => {
                let local = e.local_name().into_inner().to_vec();
                match local.as_slice() {
                    b"table-of-content-source" => {
                        source_outline_level =
                            local_attr_val(e, b"outline-level")
                                .and_then(|s| s.parse().ok())
                                .unwrap_or(3);
                        drop(e);
                        skip_element(reader)?;
                    }
                    b"index-body" => {
                        drop(e);
                        read_index_body(reader, &mut body_paragraphs)?;
                    }
                    _ => {
                        drop(e);
                        skip_element(reader)?;
                    }
                }
            }
            Ok(Event::Empty(ref e)) => {
                if e.local_name().into_inner() == b"table-of-content-source" {
                    source_outline_level =
                        local_attr_val(e, b"outline-level")
                            .and_then(|s| s.parse().ok())
                            .unwrap_or(3);
                }
            }
            Ok(Event::End(_) | Event::Eof) => break,
            Err(e) => {
                return Err(OdfError::Xml {
                    part: "content.xml".to_string(),
                    source: e,
                })
            }
            _ => {}
        }
    }
    Ok(OdfTableOfContent { name, source_outline_level, body_paragraphs })
}

/// Read `text:p` / `text:h` paragraphs inside `text:index-body`.
fn read_index_body(
    reader: &mut Reader<&[u8]>,
    paragraphs: &mut Vec<OdfParagraph>,
) -> OdfResult<()> {
    let mut buf = Vec::new();
    loop {
        buf.clear();
        match reader.read_event_into(&mut buf) {
            Ok(Event::Start(ref e)) => {
                let local = e.local_name().into_inner();
                if local == b"p" || local == b"h" {
                    paragraphs.push(read_paragraph(reader, e)?);
                } else {
                    drop(e);
                    skip_element(reader)?;
                }
            }
            Ok(Event::End(_) | Event::Eof) => break,
            Err(e) => {
                return Err(OdfError::Xml {
                    part: "content.xml".to_string(),
                    source: e,
                })
            }
            _ => {}
        }
    }
    Ok(())
}

// ── Section ───────────────────────────────────────────────────────────────────

/// Parse a `text:section` element. ODF 1.3 §5.4.
fn read_section(
    reader: &mut Reader<&[u8]>,
    tag: &BytesStart<'_>,
) -> OdfResult<OdfSection> {
    let name = local_attr_val(tag, b"name");
    let style_name = local_attr_val(tag, b"style-name");
    drop(tag);
    let children = read_body_children(reader, b"section")?;
    Ok(OdfSection { name, style_name, children })
}

// ── Body children shared dispatcher ──────────────────────────────────────────

/// Read body-level children until the `End` event whose local name matches
/// `end_tag`.
///
/// Dispatches `text:p`, `text:h`, `text:list`, `table:table`,
/// `text:table-of-content`, `text:section`, and silently skips everything
/// else. ODF 1.3 §3.1.
fn read_body_children(
    reader: &mut Reader<&[u8]>,
    end_tag: &[u8],
) -> OdfResult<Vec<OdfBodyChild>> {
    let mut children: Vec<OdfBodyChild> = Vec::new();
    let mut buf = Vec::new();
    loop {
        buf.clear();
        match reader.read_event_into(&mut buf) {
            Ok(Event::Start(ref e)) => {
                let local = e.local_name().into_inner().to_vec();
                match local.as_slice() {
                    b"p" => {
                        let para = read_paragraph(reader, e)?;
                        children.push(OdfBodyChild::Paragraph(para));
                    }
                    b"h" => {
                        let para = read_paragraph(reader, e)?;
                        children.push(OdfBodyChild::Heading(para));
                    }
                    b"list" => {
                        let list = read_list(reader, e, None, 0)?;
                        children.push(OdfBodyChild::List(list));
                    }
                    b"table" => {
                        let table = read_table(reader, e)?;
                        children.push(OdfBodyChild::Table(table));
                    }
                    b"table-of-content" => {
                        let toc = read_toc(reader, e)?;
                        children.push(OdfBodyChild::TableOfContent(toc));
                    }
                    b"section" => {
                        let section = read_section(reader, e)?;
                        children.push(OdfBodyChild::Section(section));
                    }
                    _ => {
                        drop(e);
                        skip_element(reader)?;
                    }
                }
            }
            Ok(Event::Empty(ref e)) => {
                // text:soft-page-break may appear between block elements
                let local = e.local_name().into_inner();
                if local != b"soft-page-break" {
                    // ignore other empty block-level elements
                }
            }
            Ok(Event::End(ref e)) => {
                if e.local_name().into_inner() == end_tag {
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
    Ok(children)
}

// ── Document entry point ──────────────────────────────────────────────────────

/// Parse `content.xml` bytes and return the top-level [`OdfDocument`].
///
/// Reads the `office:version` attribute from `office:document-content` (or
/// `office:document`) and the body children from `office:text`. All other
/// top-level sections (`office:automatic-styles`, `office:font-face-decls`,
/// etc.) are skipped here — they are read separately by the importer via
/// [`super::styles::read_stylesheet`] and [`super::styles::read_auto_styles`].
///
/// ODF 1.3 §3.1.
pub(crate) fn read_document(xml: &[u8]) -> OdfResult<OdfDocument> {
    let mut reader = Reader::from_reader(xml);
    reader.config_mut().trim_text(false);
    let mut buf = Vec::new();
    let mut version = OdfVersion::V1_1;
    let mut version_was_absent = true;
    let mut body_children: Vec<OdfBodyChild> = Vec::new();

    loop {
        buf.clear();
        match reader.read_event_into(&mut buf) {
            Ok(Event::Start(ref e)) => {
                let local = e.local_name().into_inner().to_vec();
                match local.as_slice() {
                    b"document-content" | b"document" => {
                        if let Some(v) = local_attr_val(e, b"version") {
                            version_was_absent = false;
                            version = OdfVersion::from_attr(&v)
                                .unwrap_or(OdfVersion::V1_3);
                        }
                        // do not skip — descend into children
                    }
                    b"text" => {
                        // office:text — the document body
                        drop(e);
                        body_children =
                            read_body_children(&mut reader, b"text")?;
                    }
                    // office:body is a thin wrapper around office:text;
                    // descend without skipping
                    b"body" => {}
                    // Skip all other top-level sections
                    _ => {
                        drop(e);
                        skip_element(&mut reader)?;
                    }
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

    Ok(OdfDocument { version, version_was_absent, body_children })
}

// ── Tests ──────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use quick_xml::events::Event;
    use quick_xml::Reader;

    use crate::odt::model::document::{OdfBodyChild, OdfListItemChild};
    use crate::odt::model::frames::OdfFrameKind;
    use crate::odt::model::notes::OdfNoteClass;
    use crate::version::OdfVersion;

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

    // ── Body-level tests ──────────────────────────────────────────────────────

    #[test]
    fn table_2x2_with_covered_cell() {
        let xml = br#"<?xml version="1.0"?>
<root xmlns:table="urn:oasis:names:tc:opendocument:xmlns:table:1.0"
      xmlns:text="urn:oasis:names:tc:opendocument:xmlns:text:1.0">
  <table:table table:name="T1">
    <table:table-column/>
    <table:table-column/>
    <table:table-row>
      <table:table-cell table:number-columns-spanned="1">
        <text:p>Cell A1</text:p>
      </table:table-cell>
      <table:table-cell>
        <text:p>Cell A2</text:p>
      </table:table-cell>
    </table:table-row>
    <table:table-row>
      <table:table-cell>
        <text:p>Cell B1</text:p>
      </table:table-cell>
      <table:covered-table-cell/>
    </table:table-row>
  </table:table>
</root>"#;
        let mut reader = Reader::from_reader(xml.as_ref());
        reader.config_mut().trim_text(false);
        let mut buf = Vec::new();
        let table = loop {
            buf.clear();
            match reader.read_event_into(&mut buf).unwrap() {
                Event::Start(ref e)
                    if e.local_name().into_inner() == b"table" =>
                {
                    break read_table(&mut reader, e).unwrap();
                }
                Event::Eof => panic!("no table found"),
                _ => {}
            }
        };
        assert_eq!(table.name.as_deref(), Some("T1"));
        assert_eq!(table.col_defs.len(), 2);
        assert_eq!(table.rows.len(), 2);

        let row0 = &table.rows[0];
        assert_eq!(row0.cells.len(), 2);
        assert!(!row0.cells[0].is_covered);
        assert_eq!(row0.cells[0].col_span, 1);
        match &row0.cells[0].paragraphs[0].children[0] {
            OdfParagraphChild::Text(s) => assert_eq!(s, "Cell A1"),
            other => panic!("{:?}", other),
        }

        let row1 = &table.rows[1];
        assert_eq!(row1.cells.len(), 2);
        assert!(!row1.cells[0].is_covered);
        assert!(row1.cells[1].is_covered);
    }

    #[test]
    fn list_with_nesting() {
        let xml = br#"<?xml version="1.0"?>
<root xmlns:text="urn:oasis:names:tc:opendocument:xmlns:text:1.0">
  <text:list text:style-name="List1">
    <text:list-item>
      <text:p>Item 1</text:p>
      <text:list>
        <text:list-item>
          <text:p>Item 1.1</text:p>
        </text:list-item>
      </text:list>
    </text:list-item>
  </text:list>
</root>"#;
        let mut reader = Reader::from_reader(xml.as_ref());
        reader.config_mut().trim_text(false);
        let mut buf = Vec::new();
        let list = loop {
            buf.clear();
            match reader.read_event_into(&mut buf).unwrap() {
                Event::Start(ref e)
                    if e.local_name().into_inner() == b"list" =>
                {
                    break read_list(&mut reader, e, None, 0).unwrap();
                }
                Event::Eof => panic!("no list found"),
                _ => {}
            }
        };
        assert_eq!(list.style_name.as_deref(), Some("List1"));
        assert_eq!(list.items.len(), 1);
        let item = &list.items[0];
        // children: Paragraph("Item 1"), List(nested)
        assert_eq!(item.children.len(), 2);
        match &item.children[0] {
            OdfListItemChild::Paragraph(p) => {
                assert_eq!(
                    p.list_context.as_ref().unwrap().level,
                    0
                );
                match &p.children[0] {
                    OdfParagraphChild::Text(s) => assert_eq!(s, "Item 1"),
                    other => panic!("{:?}", other),
                }
            }
            other => panic!("expected Paragraph, got {:?}", other),
        }
        match &item.children[1] {
            OdfListItemChild::List(nested) => {
                assert_eq!(nested.items.len(), 1);
                match &nested.items[0].children[0] {
                    OdfListItemChild::Paragraph(p) => {
                        assert_eq!(
                            p.list_context.as_ref().unwrap().level,
                            1
                        );
                    }
                    other => panic!("{:?}", other),
                }
            }
            other => panic!("expected nested List, got {:?}", other),
        }
    }

    #[test]
    fn read_document_version_present() {
        let xml = br#"<?xml version="1.0"?>
<office:document-content
  xmlns:office="urn:oasis:names:tc:opendocument:xmlns:office:1.0"
  xmlns:text="urn:oasis:names:tc:opendocument:xmlns:text:1.0"
  office:version="1.2">
  <office:body>
    <office:text>
      <text:p text:style-name="Standard">Hello</text:p>
    </office:text>
  </office:body>
</office:document-content>"#;
        let doc = read_document(xml).unwrap();
        assert_eq!(doc.version, OdfVersion::V1_2);
        assert!(!doc.version_was_absent);
        assert_eq!(doc.body_children.len(), 1);
        assert!(matches!(
            &doc.body_children[0],
            OdfBodyChild::Paragraph(p) if p.style_name.as_deref() == Some("Standard")
        ));
    }

    #[test]
    fn read_document_version_absent() {
        let xml = br#"<?xml version="1.0"?>
<office:document-content
  xmlns:office="urn:oasis:names:tc:opendocument:xmlns:office:1.0"
  xmlns:text="urn:oasis:names:tc:opendocument:xmlns:text:1.0">
  <office:body>
    <office:text>
      <text:p>No version</text:p>
    </office:text>
  </office:body>
</office:document-content>"#;
        let doc = read_document(xml).unwrap();
        assert_eq!(doc.version, OdfVersion::V1_1);
        assert!(doc.version_was_absent);
        assert_eq!(doc.body_children.len(), 1);
    }

    #[test]
    fn toc_parsing() {
        let xml = br#"<?xml version="1.0"?>
<root xmlns:text="urn:oasis:names:tc:opendocument:xmlns:text:1.0">
  <text:table-of-content text:name="TOC1">
    <text:table-of-content-source text:outline-level="2"/>
    <text:index-body>
      <text:p>Entry one</text:p>
      <text:p>Entry two</text:p>
    </text:index-body>
  </text:table-of-content>
</root>"#;
        let mut reader = Reader::from_reader(xml.as_ref());
        reader.config_mut().trim_text(false);
        let mut buf = Vec::new();
        let toc = loop {
            buf.clear();
            match reader.read_event_into(&mut buf).unwrap() {
                Event::Start(ref e)
                    if e.local_name().into_inner() == b"table-of-content" =>
                {
                    break read_toc(&mut reader, e).unwrap();
                }
                Event::Eof => panic!("no toc found"),
                _ => {}
            }
        };
        assert_eq!(toc.name.as_deref(), Some("TOC1"));
        assert_eq!(toc.source_outline_level, 2);
        assert_eq!(toc.body_paragraphs.len(), 2);
        match &toc.body_paragraphs[0].children[0] {
            OdfParagraphChild::Text(s) => assert_eq!(s, "Entry one"),
            other => panic!("{:?}", other),
        }
    }
}
