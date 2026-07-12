// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 AppThere Loki contributors

//! Leaf inline helpers for `word/document.xml` (split from `document.rs` for
//! the 300-line ceiling): bookmark start/end markers, the inline picture
//! drawing (`w:drawing`/`wp:inline`/`pic:pic`), and the alt-text flattener.
//! Called only by `write_inline` in the sibling `inlines` module.

use quick_xml::Writer;

use loki_doc_model::content::inline::Inline;

use crate::docx::write::collector::ExportCollector;
use crate::docx::write::xml::{NS_A, NS_PIC, NS_R, write_empty, write_end, write_start};

pub(super) fn write_bookmark<W: std::io::Write>(
    w: &mut Writer<W>,
    kind: loki_doc_model::content::inline::BookmarkKind,
    name: &str,
    collector: &mut ExportCollector,
) {
    use loki_doc_model::content::inline::BookmarkKind;
    // The same numeric ID appears on `w:bookmarkStart` and its paired
    // `w:bookmarkEnd` (§17.13.6.2): allocate on Start, look up on End (LIFO).
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

#[allow(clippy::unnecessary_wraps)] // Pre-existing API — changing return type would ripple to callers
#[allow(clippy::similar_names)] // cx_s / cy_s — pre-existing naming
pub(super) fn write_inline_drawing<W: std::io::Write>(
    w: &mut Writer<W>,
    r_id: &str,
    cx: u64,
    cy: u64,
    alt: &str,
) -> quick_xml::Result<()> {
    let _ = write_start(w, "w:r", &[]);
    let _ = write_start(w, "w:drawing", &[]);
    let _ = write_start(
        w,
        "wp:inline",
        &[
            ("distT", "0"),
            ("distB", "0"),
            ("distL", "0"),
            ("distR", "0"),
        ],
    );

    let cx_s = cx.to_string();
    let cy_s = cy.to_string();
    let _ = write_empty(w, "wp:extent", &[("cx", &cx_s), ("cy", &cy_s)]);
    let _ = write_empty(
        w,
        "wp:docPr",
        &[("id", "1"), ("name", "Image"), ("descr", alt)],
    );

    let _ = write_start(w, "a:graphic", &[("xmlns:a", NS_A)]);
    let _ = write_start(
        w,
        "a:graphicData",
        &[(
            "uri",
            "http://schemas.openxmlformats.org/drawingml/2006/picture",
        )],
    );

    let _ = write_start(w, "pic:pic", &[("xmlns:pic", NS_PIC)]);

    // pic:nvPicPr
    let _ = write_start(w, "pic:nvPicPr", &[]);
    let _ = write_empty(w, "pic:cNvPr", &[("id", "0"), ("name", "")]);
    let _ = write_empty(w, "pic:cNvPicPr", &[]);
    let _ = write_end(w, "pic:nvPicPr");

    // pic:blipFill
    let _ = write_start(w, "pic:blipFill", &[]);
    let _ = write_empty(w, "a:blip", &[("r:embed", r_id), ("xmlns:r", NS_R)]);
    let _ = write_start(w, "a:stretch", &[]);
    let _ = write_empty(w, "a:fillRect", &[]);
    let _ = write_end(w, "a:stretch");
    let _ = write_end(w, "pic:blipFill");

    // pic:spPr
    let _ = write_start(w, "pic:spPr", &[]);
    let _ = write_start(w, "a:xfrm", &[]);
    let _ = write_empty(w, "a:off", &[("x", "0"), ("y", "0")]);
    let _ = write_empty(w, "a:ext", &[("cx", &cx_s), ("cy", &cy_s)]);
    let _ = write_end(w, "a:xfrm");
    let _ = write_start(w, "a:prstGeom", &[("prst", "rect")]);
    let _ = write_empty(w, "a:avLst", &[]);
    let _ = write_end(w, "a:prstGeom");
    let _ = write_end(w, "pic:spPr");

    let _ = write_end(w, "pic:pic");
    let _ = write_end(w, "a:graphicData");
    let _ = write_end(w, "a:graphic");
    let _ = write_end(w, "wp:inline");
    let _ = write_end(w, "w:drawing");
    let _ = write_end(w, "w:r");

    Ok(())
}

pub(super) fn inlines_to_string(inlines: &[Inline]) -> String {
    let mut s = String::new();
    for inline in inlines {
        match inline {
            // Math carries MathML markup, not display text — excluded here.
            Inline::Str(t) | Inline::Code(_, t) => s.push_str(t),
            Inline::Space | Inline::SoftBreak => s.push(' '),
            Inline::LineBreak => s.push('\n'),
            Inline::Strong(i)
            | Inline::Emph(i)
            | Inline::Underline(i)
            | Inline::Strikeout(i)
            | Inline::Superscript(i)
            | Inline::Subscript(i)
            | Inline::SmallCaps(i)
            | Inline::Quoted(_, i)
            | Inline::Cite(_, i)
            | Inline::Span(_, i)
            | Inline::Link(_, i, _)
            | Inline::Image(_, i, _) => s.push_str(&inlines_to_string(i)),
            Inline::StyledRun(run) => s.push_str(&inlines_to_string(&run.content)),
            _ => {}
        }
    }
    s
}
