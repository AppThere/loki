// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 AppThere Loki contributors

//! Table-of-contents generation (References tab, Spec 04 M5 / plan 4a.2).
//!
//! Pure, format-neutral builders that turn a document's [`Block::Heading`]s into
//! a [`TableOfContentsBlock`] whose `body` is a cached snapshot of indented entry
//! paragraphs — the model the CRDT insert/refresh mutations write and the layout
//! engine flows. Page numbers are **not** included: they require the paginated
//! layout (a `loki-layout` concern), so the snapshot carries entry text only,
//! matching a freshly-inserted Word TOC field before it is updated against pages.

use loki_primitives::units::Points;

use crate::content::attr::NodeAttr;
use crate::content::block::{Block, StyledParagraph, TableOfContentsBlock};
use crate::content::inline::Inline;
use crate::layout::section::Section;
use crate::style::props::para_props::ParaProps;

/// The default deepest heading level a generated TOC includes (Word's default).
pub const DEFAULT_TOC_DEPTH: u8 = 3;

/// The per-level indent step for TOC entries, in points (¼ inch, like Word's
/// built-in `TOC 1`…`TOC 3` styles).
const TOC_INDENT_STEP_PT: f64 = 18.0;

/// Flattens inline content to its plain text, dropping formatting and any
/// structured objects (fields, notes, images) that carry no display string.
///
/// Used to derive a heading's TOC-entry label. Recurses through the wrapper and
/// styled-run variants; `Space`/`SoftBreak`/`LineBreak` become a single space.
#[must_use]
pub fn inline_plain_text(inlines: &[Inline]) -> String {
    let mut out = String::new();
    push_inlines(&mut out, inlines);
    out
}

fn push_inlines(out: &mut String, inlines: &[Inline]) {
    for inline in inlines {
        match inline {
            Inline::Str(s) | Inline::Code(_, s) => out.push_str(s),
            Inline::Space | Inline::SoftBreak | Inline::LineBreak => out.push(' '),
            Inline::Emph(inner)
            | Inline::Underline(inner)
            | Inline::Strong(inner)
            | Inline::Strikeout(inner)
            | Inline::Superscript(inner)
            | Inline::Subscript(inner)
            | Inline::SmallCaps(inner)
            | Inline::Quoted(_, inner)
            | Inline::Span(_, inner)
            | Inline::Link(_, inner, _)
            | Inline::Image(_, inner, _)
            | Inline::Cite(_, inner) => push_inlines(out, inner),
            Inline::StyledRun(run) => push_inlines(out, &run.content),
            // Fields, math, notes, comments, bookmarks, raw inlines carry no
            // heading-label text.
            _ => {}
        }
    }
}

/// The document's heading outline: `(level, label)` for every [`Block::Heading`]
/// with `1 <= level <= max_depth`, in document order across all sections.
#[must_use]
pub fn heading_outline(sections: &[Section], max_depth: u8) -> Vec<(u8, String)> {
    let mut out = Vec::new();
    for section in sections {
        for block in &section.blocks {
            if let Block::Heading(level, _, inlines) = block
                && *level >= 1
                && *level <= max_depth
            {
                out.push((*level, inline_plain_text(inlines)));
            }
        }
    }
    out
}

/// Builds one TOC entry paragraph: the heading `label`, indented by its `level`.
fn toc_entry(level: u8, label: &str) -> Block {
    let indent = f64::from(level.saturating_sub(1)) * TOC_INDENT_STEP_PT;
    Block::StyledPara(StyledParagraph {
        style_id: None,
        direct_para_props: Some(Box::new(ParaProps {
            indent_start: Some(Points::new(indent)),
            ..ParaProps::default()
        })),
        direct_char_props: None,
        inlines: vec![Inline::Str(label.to_string())],
        attr: NodeAttr::default(),
    })
}

/// Builds a [`TableOfContentsBlock`] from the document's headings (down to
/// `max_depth`), with an optional `title` shown above the entries.
///
/// The `title` is the localised heading text supplied by the caller (kept out of
/// the model so no user-visible string is hardcoded); it is baked into the body
/// as a bold paragraph rather than an outline [`Block::Heading`], so a later
/// refresh does not pick the TOC's own title up as an entry.
#[must_use]
pub fn build_toc(sections: &[Section], title: Option<&str>, max_depth: u8) -> TableOfContentsBlock {
    let mut body = Vec::new();
    if let Some(title) = title.filter(|t| !t.is_empty()) {
        body.push(Block::StyledPara(StyledParagraph {
            style_id: None,
            direct_para_props: None,
            direct_char_props: None,
            inlines: vec![Inline::Strong(vec![Inline::Str(title.to_string())])],
            attr: NodeAttr::default(),
        }));
    }
    for (level, label) in heading_outline(sections, max_depth) {
        body.push(toc_entry(level, &label));
    }
    TableOfContentsBlock {
        title: None,
        body,
        attr: NodeAttr::default(),
    }
}

#[cfg(test)]
#[path = "toc_tests.rs"]
mod tests;
