// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 AppThere Loki contributors

//! Round-trip canonicalization for the suite's format-neutral document model.
//!
//! Implements [`NormalizedModel`](crate::roundtrip::NormalizedModel) for
//! `loki_doc_model::Document` (feature `doc-model`), so the round-trip axis works
//! on real documents. The canonical form is an order-stable sequence of
//! `(path, value)` entries that captures the *semantically significant* content
//! — paragraph structure and text, run-level character formatting (direct
//! [`CharProps`] and the Pandoc emphasis wrappers), style references, and
//! **bookmark ids** (the named round-trip class, Spec 02 §6) — while ignoring
//! incidental differences. [`crate::roundtrip::first_divergence`] then pinpoints
//! the first loss with a model path.
//!
//! Coverage is the common word-processing shapes (paragraphs, headings, styled
//! paragraphs, lists, quotes, runs, links, bookmarks); other block/inline kinds
//! are recorded by *kind* (so a structural change is still caught) and deepened
//! in a follow-up. Document metadata and table interiors are likewise a
//! follow-up. The shape is extensible: each pass adds entries, never changes the
//! comparison engine.

use loki_doc_model::Document;
use loki_doc_model::content::block::Block;
use loki_doc_model::content::inline::Inline;
use loki_doc_model::style::props::char_props::CharProps;

use crate::roundtrip::{CanonicalEntry, NormalizedModel};

mod meta;
mod tables;

impl NormalizedModel for Document {
    fn canonical(&self) -> Vec<CanonicalEntry> {
        canonicalize_document(self)
    }
}

/// Produces the canonical `(path, value)` form of `doc` (see the module docs).
#[must_use]
pub fn canonicalize_document(doc: &Document) -> Vec<CanonicalEntry> {
    let mut out = Vec::new();
    meta::canonicalize_meta(&doc.meta, &mut out);
    for (si, section) in doc.sections.iter().enumerate() {
        for (bi, block) in section.blocks.iter().enumerate() {
            walk_block(block, &format!("sec{si:04}/blk{bi:04}"), &mut out);
        }
    }
    out
}

fn push(out: &mut Vec<CanonicalEntry>, path: String, value: impl Into<String>) {
    out.push(CanonicalEntry::new(path, value));
}

fn walk_block(block: &Block, path: &str, out: &mut Vec<CanonicalEntry>) {
    push(out, format!("{path}/kind"), block_kind(block));
    match block {
        Block::Plain(inlines) | Block::Para(inlines) => walk_inlines(inlines, path, out),
        Block::Heading(level, _, inlines) => {
            push(out, format!("{path}/level"), level.to_string());
            walk_inlines(inlines, path, out);
        }
        Block::StyledPara(sp) => {
            if let Some(id) = &sp.style_id {
                push(out, format!("{path}/style"), id.0.clone());
            }
            if let Some(props) = &sp.direct_char_props {
                push(out, format!("{path}/mark_props"), char_summary(props));
            }
            walk_inlines(&sp.inlines, path, out);
        }
        Block::BlockQuote(blocks) => {
            for (i, b) in blocks.iter().enumerate() {
                walk_block(b, &format!("{path}/q{i:04}"), out);
            }
        }
        Block::OrderedList(_, items) | Block::BulletList(items) => {
            for (li, item) in items.iter().enumerate() {
                for (bi, b) in item.iter().enumerate() {
                    walk_block(b, &format!("{path}/li{li:04}/{bi:04}"), out);
                }
            }
        }
        Block::CodeBlock(_, code) | Block::RawBlock(_, code) => {
            push(out, format!("{path}/text"), code.clone());
        }
        Block::Table(tbl) => tables::walk_table(tbl, path, out),
        // Other kinds: recorded by `kind` above; deep walk is a follow-up.
        _ => {}
    }
}

fn walk_inlines(inlines: &[Inline], path: &str, out: &mut Vec<CanonicalEntry>) {
    for (i, inl) in inlines.iter().enumerate() {
        walk_inline(inl, &format!("{path}/i{i:04}"), out);
    }
}

fn walk_inline(inl: &Inline, path: &str, out: &mut Vec<CanonicalEntry>) {
    push(out, format!("{path}/kind"), inline_kind(inl));
    match inl {
        Inline::Str(s) => push(out, format!("{path}/str"), s.clone()),
        // Emphasis wrappers: the `kind` above records the mark; recurse content.
        Inline::Emph(c)
        | Inline::Underline(c)
        | Inline::Strong(c)
        | Inline::Strikeout(c)
        | Inline::Superscript(c)
        | Inline::Subscript(c)
        | Inline::SmallCaps(c) => walk_inlines(c, path, out),
        Inline::StyledRun(sr) => {
            if let Some(id) = &sr.style_id {
                push(out, format!("{path}/style"), id.0.clone());
            }
            if let Some(props) = &sr.direct_props {
                push(out, format!("{path}/props"), char_summary(props));
            }
            walk_inlines(&sr.content, path, out);
        }
        Inline::Bookmark(kind, id) => {
            push(out, format!("{path}/id"), format!("{kind:?}:{id}"));
        }
        Inline::Link(_, c, _)
        | Inline::Image(_, c, _)
        | Inline::Span(_, c)
        | Inline::Quoted(_, c) => {
            walk_inlines(c, path, out);
        }
        // Space / breaks / fields / math / notes / etc.: `kind` is enough here.
        _ => {}
    }
}

/// A compact, stable serialization of the *set* direct character properties.
/// Only `Some` fields appear, in a fixed order, so dropping any one property
/// changes the string — and the round-trip differ catches it.
fn char_summary(p: &CharProps) -> String {
    let mut parts: Vec<String> = Vec::new();
    if let Some(v) = &p.font_name {
        parts.push(format!("font={v}"));
    }
    if let Some(v) = &p.font_size {
        parts.push(format!("size={v:?}"));
    }
    if let Some(v) = p.bold {
        parts.push(format!("bold={v}"));
    }
    if let Some(v) = p.italic {
        parts.push(format!("italic={v}"));
    }
    if let Some(v) = &p.underline {
        parts.push(format!("underline={v:?}"));
    }
    if let Some(v) = &p.strikethrough {
        parts.push(format!("strike={v:?}"));
    }
    if let Some(v) = p.small_caps {
        parts.push(format!("smallcaps={v}"));
    }
    if let Some(v) = &p.vertical_align {
        parts.push(format!("valign={v:?}"));
    }
    if let Some(v) = &p.color {
        parts.push(format!("color={v:?}"));
    }
    parts.join(";")
}

fn block_kind(b: &Block) -> &'static str {
    match b {
        Block::Plain(_) => "plain",
        Block::Para(_) => "para",
        Block::LineBlock(_) => "lineblock",
        Block::CodeBlock(_, _) => "codeblock",
        Block::RawBlock(_, _) => "rawblock",
        Block::BlockQuote(_) => "blockquote",
        Block::OrderedList(_, _) => "orderedlist",
        Block::BulletList(_) => "bulletlist",
        Block::DefinitionList(_) => "definitionlist",
        Block::Heading(_, _, _) => "heading",
        Block::HorizontalRule => "horizontalrule",
        Block::Table(_) => "table",
        Block::Figure(_, _, _) => "figure",
        Block::Div(_, _) => "div",
        Block::StyledPara(_) => "styledpara",
        Block::TableOfContents(_) => "tableofcontents",
        Block::Index(_) => "index",
        Block::NotesBlock(_) => "notesblock",
        // `Block` is `#[non_exhaustive]`; track any future variant by a marker.
        _ => "unknown",
    }
}

fn inline_kind(i: &Inline) -> &'static str {
    match i {
        Inline::Str(_) => "str",
        Inline::Emph(_) => "emph",
        Inline::Underline(_) => "underline",
        Inline::Strong(_) => "strong",
        Inline::Strikeout(_) => "strikeout",
        Inline::Superscript(_) => "superscript",
        Inline::Subscript(_) => "subscript",
        Inline::SmallCaps(_) => "smallcaps",
        Inline::Quoted(_, _) => "quoted",
        Inline::Cite(_, _) => "cite",
        Inline::Code(_, _) => "code",
        Inline::Space => "space",
        Inline::SoftBreak => "softbreak",
        Inline::LineBreak => "linebreak",
        Inline::Math(_, _) => "math",
        Inline::RawInline(_, _) => "rawinline",
        Inline::Link(_, _, _) => "link",
        Inline::Image(_, _, _) => "image",
        Inline::Note(_, _) => "note",
        Inline::Span(_, _) => "span",
        Inline::StyledRun(_) => "styledrun",
        Inline::Field(_) => "field",
        Inline::Comment(_) => "comment",
        Inline::Bookmark(_, _) => "bookmark",
        // `Inline` is `#[non_exhaustive]`; record any future variant by a marker
        // so its presence is still tracked (and deepened in a follow-up).
        _ => "unknown",
    }
}

#[cfg(test)]
#[path = "model_tests.rs"]
mod tests;
