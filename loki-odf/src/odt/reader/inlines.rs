// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 AppThere Loki contributors

//! Inline-level parsing for `content.xml` (`text:span`, `text:a`, fields,
//! bookmarks, frames, notes), split out of [`super::document`].
//!
//! [`read_inline_children`] recurses for nested `text:span` / `text:a`
//! elements. The recursion is bounded by [`MAX_NESTING_DEPTH`], and the
//! non-recursive element handling lives in separate helper functions so the
//! recursive frame stays small (deeply nested hostile input must hit the
//! typed depth error, not exhaust the stack first).

use quick_xml::Reader;
use quick_xml::events::{BytesStart, Event};

use crate::error::{OdfError, OdfResult};
use crate::limits::{MAX_NESTING_DEPTH, MAX_REPEATED_SPACES};
use crate::odt::model::fields::OdfField;
use crate::odt::model::frames::OdfFrame;
use crate::odt::model::notes::OdfNoteClass;
use crate::odt::model::paragraph::{OdfHyperlink, OdfParagraphChild, OdfSpan};
use crate::xml_util::local_attr_val;

use super::document::{read_frame_kind, read_note_body, skip_element};

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
///
/// `depth` is the current span/hyperlink nesting depth (0 at the paragraph
/// level); recursion beyond [`MAX_NESTING_DEPTH`] returns
/// [`OdfError::NestingTooDeep`] instead of overflowing the stack.
pub(crate) fn read_inline_children(
    reader: &mut Reader<&[u8]>,
    depth: usize,
) -> OdfResult<Vec<OdfParagraphChild>> {
    if depth > MAX_NESTING_DEPTH {
        return Err(OdfError::NestingTooDeep {
            limit: MAX_NESTING_DEPTH,
        });
    }
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
                        let span_children = read_inline_children(reader, depth.saturating_add(1))?;
                        children.push(OdfParagraphChild::Span(OdfSpan {
                            style_name,
                            children: span_children,
                        }));
                    }
                    // Hyperlink — ODF 1.3 §6.4
                    b"a" => {
                        let href = local_attr_val(e, b"href");
                        let style_name = local_attr_val(e, b"style-name");
                        let link_children = read_inline_children(reader, depth.saturating_add(1))?;
                        children.push(OdfParagraphChild::Hyperlink(OdfHyperlink {
                            href,
                            style_name,
                            children: link_children,
                        }));
                    }
                    // All non-recursive inline elements are handled out of
                    // line so this recursive frame stays small.
                    _ => read_inline_start_other(reader, e, &local, &mut children)?,
                }
            }
            // ── Empty elements ─────────────────────────────────────────────
            Ok(Event::Empty(ref e)) => children.push(inline_from_empty(e)),
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

/// Handle a non-recursive inline `Start` element (note, frame, bookmark,
/// or text field), pushing the parsed child onto `children`.
fn read_inline_start_other(
    reader: &mut Reader<&[u8]>,
    e: &BytesStart<'_>,
    local: &[u8],
    children: &mut Vec<OdfParagraphChild>,
) -> OdfResult<()> {
    match local {
        // Footnote / endnote — ODF 1.3 §6.3
        b"note" => {
            let id = local_attr_val(e, b"id");
            let note_class = match local_attr_val(e, b"note-class").as_deref() {
                Some("endnote") => OdfNoteClass::Endnote,
                _ => OdfNoteClass::Footnote,
            };
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
            skip_element(reader)?;
            children.push(OdfParagraphChild::Bookmark { id, name });
        }
        // Bookmark range-end — ODF 1.3 §6.6
        b"bookmark-end" => {
            let name = local_attr_val(e, b"name").unwrap_or_default();
            skip_element(reader)?;
            children.push(OdfParagraphChild::BookmarkEnd { name });
        }
        // ── Text fields ────────────────────────────────────────────────────
        // Field attributes are extracted before the wrapping element (which
        // carries only the field's current display text) is skipped.
        _ => {
            let field = field_from_element(e, local);
            skip_element(reader)?;
            children.push(field);
        }
    }
    Ok(())
}

/// Handle an inline `Empty` element (`text:tab`, `text:s`, bookmarks,
/// self-closing fields), returning the parsed child.
fn inline_from_empty(e: &BytesStart<'_>) -> OdfParagraphChild {
    let local = e.local_name().into_inner();
    match local {
        b"tab" => OdfParagraphChild::Tab,
        b"line-break" => OdfParagraphChild::LineBreak,
        b"soft-page-break" => OdfParagraphChild::SoftReturn,
        b"s" => {
            // Clamp attacker-controlled space counts at parse time so no
            // downstream consumer can allocate N bytes from them.
            let count: u32 = local_attr_val(e, b"c")
                .and_then(|s| s.parse().ok())
                .unwrap_or(1)
                .min(MAX_REPEATED_SPACES);
            OdfParagraphChild::Space { count }
        }
        b"bookmark" | b"bookmark-start" => {
            let id = local_attr_val(e, b"id");
            let name = local_attr_val(e, b"name").unwrap_or_default();
            OdfParagraphChild::Bookmark { id, name }
        }
        b"bookmark-end" => {
            let name = local_attr_val(e, b"name").unwrap_or_default();
            OdfParagraphChild::BookmarkEnd { name }
        }
        _ => field_from_element(e, local),
    }
}

/// Build a field child from a field element's attributes, or
/// [`OdfParagraphChild::Other`] for unrecognised elements.
///
/// Shared between the `Start` (field content skipped by the caller) and
/// `Empty` element paths.
fn field_from_element(e: &BytesStart<'_>, local: &[u8]) -> OdfParagraphChild {
    let field = match local {
        b"page-number" => OdfField::PageNumber {
            select_page: local_attr_val(e, b"select-page"),
        },
        b"page-count" => OdfField::PageCount,
        b"date" => OdfField::Date {
            data_style: local_attr_val(e, b"data-style-name"),
            fixed_value: local_attr_val(e, b"date-value"),
        },
        b"time" => OdfField::Time {
            data_style: local_attr_val(e, b"data-style-name"),
            fixed_value: local_attr_val(e, b"time-value"),
        },
        b"title" => OdfField::Title,
        b"subject" => OdfField::Subject,
        b"author-name" => OdfField::AuthorName,
        b"file-name" => OdfField::FileName {
            display: local_attr_val(e, b"display"),
        },
        b"word-count" => OdfField::WordCount,
        b"chapter" => OdfField::ChapterName {
            display_levels: local_attr_val(e, b"display-levels")
                .and_then(|s| s.parse().ok())
                .unwrap_or(1),
        },
        b"bookmark-ref" | b"sequence-ref" => OdfField::CrossReference {
            ref_name: local_attr_val(e, b"ref-name").unwrap_or_default(),
            display: local_attr_val(e, b"reference-format")
                .or_else(|| local_attr_val(e, b"display")),
        },
        _ => return OdfParagraphChild::Other,
    };
    OdfParagraphChild::Field(field)
}
