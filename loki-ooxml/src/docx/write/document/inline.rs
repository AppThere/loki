// SPDX-License-Identifier: Apache-2.0
// Copyright (C) 2025 AppThere contributors

//! Inline-level serializers for `word/document.xml`.

use quick_xml::Writer;

use loki_doc_model::content::inline::{Inline, NoteKind, StyledRun};
use loki_doc_model::style::props::char_props::CharProps;

use crate::docx::write::collector::ExportCollector;
use crate::docx::write::styles::emit_char_props;
use crate::docx::write::xml::{write_empty, write_end, write_start, wval};

use super::drawing::{inlines_to_string, write_inline_drawing};

/// Accumulated run formatting inherited from inline wrappers.
#[allow(clippy::struct_excessive_bools)] // Pre-existing pattern — structural refactor deferred
#[derive(Default, Clone)]
pub(super) struct RunProps {
    pub bold: bool,
    pub italic: bool,
    pub underline: bool,
    pub strikethrough: bool,
    pub superscript: bool,
    pub subscript: bool,
    pub small_caps: bool,
    pub code: bool,
    pub char_style: Option<String>,
    pub direct: Option<CharProps>,
}

pub(super) fn write_inlines<W: std::io::Write>(
    w: &mut Writer<W>,
    inlines: &[Inline],
    props: &RunProps,
    collector: &mut ExportCollector,
) {
    for inline in inlines {
        write_inline(w, inline, props, collector);
    }
}

#[allow(clippy::too_many_lines)] // Pre-existing pattern — structural refactor deferred
pub(super) fn write_inline<W: std::io::Write>(
    w: &mut Writer<W>,
    inline: &Inline,
    props: &RunProps,
    collector: &mut ExportCollector,
) {
    match inline {
        Inline::Str(s) => write_text_run(w, s, props),
        Inline::Space | Inline::SoftBreak => write_text_run(w, " ", props),
        Inline::LineBreak => {
            let _ = write_start(w, "w:r", &[]);
            let _ = write_empty(w, "w:br", &[]);
            let _ = write_end(w, "w:r");
        }
        Inline::Strong(inner) => {
            let np = RunProps {
                bold: true,
                ..props.clone()
            };
            write_inlines(w, inner, &np, collector);
        }
        Inline::Emph(inner) => {
            let np = RunProps {
                italic: true,
                ..props.clone()
            };
            write_inlines(w, inner, &np, collector);
        }
        Inline::Underline(inner) => {
            let np = RunProps {
                underline: true,
                ..props.clone()
            };
            write_inlines(w, inner, &np, collector);
        }
        Inline::Strikeout(inner) => {
            let np = RunProps {
                strikethrough: true,
                ..props.clone()
            };
            write_inlines(w, inner, &np, collector);
        }
        Inline::Superscript(inner) => {
            let np = RunProps {
                superscript: true,
                ..props.clone()
            };
            write_inlines(w, inner, &np, collector);
        }
        Inline::Subscript(inner) => {
            let np = RunProps {
                subscript: true,
                ..props.clone()
            };
            write_inlines(w, inner, &np, collector);
        }
        Inline::SmallCaps(inner) => {
            let np = RunProps {
                small_caps: true,
                ..props.clone()
            };
            write_inlines(w, inner, &np, collector);
        }
        Inline::Quoted(kind, inner) => {
            use loki_doc_model::content::inline::QuoteType;
            let (open, close) = match kind {
                QuoteType::SingleQuote => ("\u{2018}", "\u{2019}"),
                QuoteType::DoubleQuote => ("\u{201C}", "\u{201D}"),
            };
            write_text_run(w, open, props);
            write_inlines(w, inner, props, collector);
            write_text_run(w, close, props);
        }
        Inline::Cite(_, inner) | Inline::Span(_, inner) => {
            write_inlines(w, inner, props, collector);
        }
        Inline::Code(_, s) => {
            let np = RunProps {
                code: true,
                ..props.clone()
            };
            write_text_run(w, s, &np);
        }
        Inline::Link(_, inner, target) => {
            let r_id = collector.add_hyperlink(&target.url);
            let _ = write_start(w, "w:hyperlink", &[("r:id", &r_id)]);
            write_inlines(w, inner, props, collector);
            let _ = write_end(w, "w:hyperlink");
        }
        Inline::StyledRun(run) => {
            write_styled_run(w, run, props, collector);
        }
        Inline::Bookmark(kind, name) => {
            write_bookmark(w, *kind, name, collector);
        }
        Inline::Math(_, s) => {
            write_text_run(w, s, props);
        }
        Inline::Image(_, inlines, target) => {
            if let Some(r_id) = collector.add_image(&target.url) {
                // Default: 1 inch = 914400 EMU.
                let alt = inlines_to_string(inlines);
                let _ = write_inline_drawing(w, &r_id, 914_400, 914_400, &alt);
            } else {
                write_text_run(w, "[Image]", props);
            }
        }
        Inline::Note(kind, blocks) => {
            let note_id = match kind {
                NoteKind::Footnote => collector.add_footnote(blocks.clone()),
                NoteKind::Endnote => collector.add_endnote(blocks.clone()),
                _ => 0,
            };

            let _ = write_start(w, "w:r", &[]);
            let _ = write_start(w, "w:rPr", &[]);
            let _ = write_empty(w, "w:vertAlign", &wval("superscript"));
            let style = match kind {
                NoteKind::Footnote => "FootnoteReference",
                NoteKind::Endnote => "EndnoteReference",
                _ => "DefaultParagraphFont",
            };
            let _ = write_empty(w, "w:rStyle", &wval(style));
            let _ = write_end(w, "w:rPr");

            let elem = match kind {
                NoteKind::Endnote => "w:endnoteReference",
                _ => "w:footnoteReference",
            };
            let _ = write_empty(w, elem, &[("w:id", &note_id.to_string())]);
            let _ = write_end(w, "w:r");
        }
        _ => {}
    }
}

fn write_styled_run<W: std::io::Write>(
    w: &mut Writer<W>,
    run: &StyledRun,
    parent: &RunProps,
    collector: &mut ExportCollector,
) {
    let np = RunProps {
        bold: parent.bold,
        italic: parent.italic,
        underline: parent.underline,
        strikethrough: parent.strikethrough,
        superscript: parent.superscript,
        subscript: parent.subscript,
        small_caps: parent.small_caps,
        code: parent.code,
        char_style: run.style_id.as_ref().map(|s| s.0.clone()),
        direct: run.direct_props.as_deref().cloned(),
    };
    write_inlines(w, &run.content, &np, collector);
}

/// Writes a single `<w:r>` element with text content.
pub(super) fn write_text_run<W: std::io::Write>(w: &mut Writer<W>, text: &str, props: &RunProps) {
    if text.is_empty() {
        return;
    }
    let _ = write_start(w, "w:r", &[]);

    // Emit w:rPr if any formatting is active.
    let has_rpr = props.bold
        || props.italic
        || props.underline
        || props.strikethrough
        || props.superscript
        || props.subscript
        || props.small_caps
        || props.code
        || props.char_style.is_some()
        || props.direct.is_some();

    if has_rpr {
        let _ = write_start(w, "w:rPr", &[]);
        if let Some(ref sid) = props.char_style {
            let _ = write_empty(w, "w:rStyle", &wval(sid));
        }
        if props.code {
            let _ = write_empty(
                w,
                "w:rFonts",
                &[("w:ascii", "Courier New"), ("w:hAnsi", "Courier New")],
            );
        }
        if props.bold {
            let _ = write_empty(w, "w:b", &[]);
        }
        if props.italic {
            let _ = write_empty(w, "w:i", &[]);
        }
        if props.small_caps {
            let _ = write_empty(w, "w:smallCaps", &[]);
        }
        if props.underline {
            let _ = write_empty(w, "w:u", &wval("single"));
        }
        if props.strikethrough {
            let _ = write_empty(w, "w:strike", &[]);
        }
        if props.superscript {
            let _ = write_empty(w, "w:vertAlign", &wval("superscript"));
        } else if props.subscript {
            let _ = write_empty(w, "w:vertAlign", &wval("subscript"));
        }
        if let Some(ref cp) = props.direct {
            emit_char_props(w, cp);
        }
        let _ = write_end(w, "w:rPr");
    }

    // Text node — always use xml:space="preserve" to keep leading/trailing spaces.
    let _ = write_empty_checked(w, text);
    let _ = write_end(w, "w:r");
}

/// Writes `<w:t xml:space="preserve">text</w:t>`.
fn write_empty_checked<W: std::io::Write>(w: &mut Writer<W>, text: &str) -> quick_xml::Result<()> {
    use quick_xml::events::{BytesStart, BytesText, Event};
    let mut start = BytesStart::new("w:t");
    start.push_attribute(("xml:space", "preserve"));
    w.write_event(Event::Start(start))?;
    w.write_event(Event::Text(BytesText::new(text)))?;
    w.write_event(Event::End(quick_xml::events::BytesEnd::new("w:t")))
}

fn write_bookmark<W: std::io::Write>(
    w: &mut Writer<W>,
    kind: loki_doc_model::content::inline::BookmarkKind,
    name: &str,
    collector: &mut ExportCollector,
) {
    use loki_doc_model::content::inline::BookmarkKind;
    // The same numeric ID must appear on both `w:bookmarkStart` and its
    // paired `w:bookmarkEnd` (ECMA-376 §17.13.6.2).  We allocate on Start
    // and look up on End via the collector's per-name LIFO stack.
    let id = match kind {
        BookmarkKind::Start => collector.assign_bookmark_id(name),
        BookmarkKind::End => collector.resolve_bookmark_id(name),
    };
    let id_s = id.to_string();
    match kind {
        BookmarkKind::Start => {
            let _ = write_empty(w, "w:bookmarkStart", &[("w:id", &id_s), ("w:name", name)]);
        }
        BookmarkKind::End => {
            let _ = write_empty(w, "w:bookmarkEnd", &[("w:id", &id_s)]);
        }
    }
}
