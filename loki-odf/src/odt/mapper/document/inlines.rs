// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 AppThere Loki contributors

//! Inline-level mapping: ODF paragraph children → [`Inline`]s.

use loki_doc_model::content::attr::NodeAttr;
use loki_doc_model::content::inline::{BookmarkKind, Inline, LinkTarget, NoteKind, StyledRun};
use loki_doc_model::style::catalog::StyleId;

use crate::odt::model::notes::{OdfNote, OdfNoteClass};
use crate::odt::model::paragraph::{OdfHyperlink, OdfParagraphChild, OdfSpan};

use super::context::OdfMappingContext;
use super::fields::map_field;
use super::frames::map_frame;
use super::paragraphs::map_paragraph;

pub(crate) fn map_inline_children(
    children: &[OdfParagraphChild],
    ctx: &mut OdfMappingContext<'_>,
) -> Vec<Inline> {
    children.iter().filter_map(|c| map_inline(c, ctx)).collect()
}

pub(crate) fn map_inline(
    child: &OdfParagraphChild,
    ctx: &mut OdfMappingContext<'_>,
) -> Option<Inline> {
    match child {
        OdfParagraphChild::Text(s) => {
            if s.is_empty() {
                None
            } else {
                Some(Inline::Str(s.clone()))
            }
        }
        OdfParagraphChild::Span(span) => Some(map_span(span, ctx)),
        OdfParagraphChild::Hyperlink(link) => Some(map_hyperlink(link, ctx)),
        OdfParagraphChild::Note(note) => Some(map_note(note, ctx)),
        OdfParagraphChild::Bookmark { name, .. } => {
            Some(Inline::Bookmark(BookmarkKind::Start, name.clone()))
        }
        OdfParagraphChild::BookmarkEnd { name } => {
            Some(Inline::Bookmark(BookmarkKind::End, name.clone()))
        }
        OdfParagraphChild::Field(field) => Some(Inline::Field(map_field(field))),
        OdfParagraphChild::Frame(frame) => map_frame(frame, ctx),
        OdfParagraphChild::SoftReturn | OdfParagraphChild::Other => None,
        OdfParagraphChild::Tab => Some(Inline::Str("\t".into())),
        OdfParagraphChild::Space { count } => Some(Inline::Str(" ".repeat(*count as usize))),
        OdfParagraphChild::LineBreak => Some(Inline::LineBreak),
    }
}

fn map_span(span: &OdfSpan, ctx: &mut OdfMappingContext<'_>) -> Inline {
    let style_id = span.style_name.as_deref().map(StyleId::new);
    let content = map_inline_children(&span.children, ctx);
    Inline::StyledRun(StyledRun {
        style_id,
        direct_props: None,
        content,
        attr: NodeAttr::default(),
    })
}

fn map_hyperlink(link: &OdfHyperlink, ctx: &mut OdfMappingContext<'_>) -> Inline {
    let href = link.href.clone().unwrap_or_default();
    let content = map_inline_children(&link.children, ctx);
    Inline::Link(NodeAttr::default(), content, LinkTarget::new(href))
}

fn map_note(note: &OdfNote, ctx: &mut OdfMappingContext<'_>) -> Inline {
    let kind = match note.note_class {
        OdfNoteClass::Footnote => NoteKind::Footnote,
        OdfNoteClass::Endnote => NoteKind::Endnote,
    };
    let body: Vec<loki_doc_model::content::block::Block> = note
        .body
        .iter()
        .flat_map(|p| {
            let block = map_paragraph(p, ctx);
            let figs = std::mem::take(&mut ctx.pending_figures);
            std::iter::once(block).chain(figs)
        })
        .collect();
    Inline::Note(kind, body)
}
