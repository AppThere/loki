// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 AppThere Loki contributors

//! DOCX tracked-change (`w:ins` / `w:del`) export helpers (Review tab, 4a.2).
//!
//! A run whose `CharProps::revision` is set is wrapped in a `w:ins` (insertion)
//! or `w:del` (deletion) element carrying `w:id` / `w:author` / `w:date`; a
//! deletion additionally emits its text as `w:delText` instead of `w:t` (handled
//! by the run writer). This mirrors the import path in `reader::runs`.

use std::io::Write;

use loki_doc_model::style::props::revision::{RevisionKind, RevisionMark};
use quick_xml::Writer;

use super::xml::{write_empty, write_start};

/// The wrapper element name for a revision: `w:ins` or `w:del`.
#[must_use]
pub(super) fn tag(rev: &RevisionMark) -> &'static str {
    match rev.kind {
        RevisionKind::Insertion => "w:ins",
        RevisionKind::Deletion => "w:del",
    }
}

/// Writes the `w:ins` / `w:del` element for `rev` with its `w:id` / `w:author` /
/// `w:date` attributes (id defaults to `1`, author to empty, date omitted when
/// absent) — as an opening tag (`empty = false`, the run wrapper) or a
/// self-closing element (`empty = true`, the paragraph-mark marker in `w:rPr`).
fn write_rev_element<W: Write>(w: &mut Writer<W>, rev: &RevisionMark, empty: bool) {
    let id = rev.id.clone().unwrap_or_else(|| "1".to_string());
    let author = rev.author.clone().unwrap_or_default();
    let mut attrs: Vec<(&str, &str)> = vec![("w:id", id.as_str()), ("w:author", author.as_str())];
    if let Some(date) = rev.date.as_deref() {
        attrs.push(("w:date", date));
    }
    let _ = if empty {
        write_empty(w, tag(rev), &attrs)
    } else {
        write_start(w, tag(rev), &attrs)
    };
}

/// Opens the `w:ins` / `w:del` run wrapper for `rev`. The caller closes it with
/// `write_end(w, tag(rev))`.
pub(super) fn open<W: Write>(w: &mut Writer<W>, rev: &RevisionMark) {
    write_rev_element(w, rev, false);
}

/// Writes the self-closing `<w:del/>` / `<w:ins/>` marker for a tracked
/// **paragraph-mark** revision inside its `w:rPr` (OOXML §17.13.5.13 —
/// `w:pPr/w:rPr/w:del`). A no-op when `rev` is `None` or not present.
pub(super) fn write_mark_del<W: Write>(w: &mut Writer<W>, rev: Option<&RevisionMark>) {
    if let Some(rev) = rev {
        write_rev_element(w, rev, true);
    }
}

/// Writes a run's text node — `w:delText` for a tracked deletion (ECMA-376
/// §17.13.5.15), else `w:t` — always `xml:space="preserve"` to keep spaces.
pub(super) fn write_text_node<W: Write>(w: &mut Writer<W>, text: &str, rev: Option<&RevisionMark>) {
    use quick_xml::events::{BytesEnd, BytesStart, BytesText, Event};
    let tag = match rev {
        Some(r) if r.kind == RevisionKind::Deletion => "w:delText",
        _ => "w:t",
    };
    let mut start = BytesStart::new(tag);
    start.push_attribute(("xml:space", "preserve"));
    let _ = w.write_event(Event::Start(start));
    let _ = w.write_event(Event::Text(BytesText::new(text)));
    let _ = w.write_event(Event::End(BytesEnd::new(tag)));
}
