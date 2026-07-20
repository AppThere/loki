// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 AppThere Loki contributors

//! Inline/run serialization for `word/document.xml` (split from `document.rs`
//! for the 300-line ceiling): walks the `Inline` tree, emitting `w:r` runs
//! with their `w:rPr`, hyperlinks, styled runs, fields, notes, and math. The
//! `RunProps` accumulator and block-level writers stay in `document.rs`; the
//! leaf bookmark/drawing/alt-text helpers live in the sibling `drawing`
//! module. `write_text_run` is re-exported from `document.rs` for `fields`.

use quick_xml::Writer;

use loki_doc_model::content::inline::{Inline, NoteKind, StyledRun};

use super::RunProps;
use super::drawing::{
    inlines_to_string, write_anchor_drawing, write_bookmark, write_inline_drawing,
};
use super::textbox::write_textbox_drawing;
use crate::docx::write::collector::ExportCollector;
use crate::docx::write::run_props::emit_char_props;
use crate::docx::write::xml::{write_empty, write_end, write_start, wval};

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
fn write_inline<W: std::io::Write>(
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
        Inline::Math(kind, mathml) => {
            use loki_doc_model::content::inline::MathType;
            crate::docx::omml::write_omath(w, mathml, *kind == MathType::DisplayMath);
        }
        Inline::Field(field) => crate::docx::write::fields::write_field(w, field, props),
        Inline::Comment(c) => crate::docx::write::comments::write_comment_ref(w, c),
        Inline::Image(attr, inlines, target) => {
            if let Some(r_id) = collector.add_image(&target.url) {
                // Default: 1 inch = 914400 EMU.
                let alt = inlines_to_string(inlines);
                // A floating image (wrap keys or the `floating` class on its
                // node attributes) exports as `wp:anchor` so the wrap mode
                // survives; otherwise it is a plain inline picture.
                if let Some(wrap) =
                    loki_doc_model::content::float::FloatWrap::read_or_class_default(attr)
                {
                    write_anchor_drawing(w, &r_id, 914_400, 914_400, &alt, wrap);
                } else {
                    let _ = write_inline_drawing(w, &r_id, 914_400, 914_400, &alt);
                }
            } else {
                write_text_run(w, "[Image]", props);
            }
        }
        Inline::TextBox(attr, blocks) => {
            write_textbox_drawing(w, attr, blocks, collector);
        }
        Inline::Note(kind, blocks) => {
            let note_id = match kind {
                NoteKind::Footnote => collector.add_footnote(blocks.clone()),
                NoteKind::Endnote => collector.add_endnote(blocks.clone()),
                _ => 0,
            };

            let _ = write_start(w, "w:r", &[]);
            let _ = write_start(w, "w:rPr", &[]);
            // Superscript comes from the note-reference char style (styles.rs).
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
    // A tracked run is wrapped in w:ins/w:del (its text emits as w:delText).
    if let Some(rev) = run.direct_props.as_ref().and_then(|p| p.revision.clone()) {
        crate::docx::write::revision::open(w, &rev);
        write_inlines(w, &run.content, &np, collector);
        let _ = write_end(w, crate::docx::write::revision::tag(&rev));
    } else {
        write_inlines(w, &run.content, &np, collector);
    }
}

/// Writes a single `<w:r>` element with text content.
pub(crate) fn write_text_run<W: std::io::Write>(w: &mut Writer<W>, text: &str, props: &RunProps) {
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

    let rev = props.direct.as_ref().and_then(|p| p.revision.as_ref());
    crate::docx::write::revision::write_text_node(w, text, rev);
    let _ = write_end(w, "w:r");
}
