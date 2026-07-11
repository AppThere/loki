// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 AppThere Loki contributors

//! Paragraph, inline, and field mapping.
//!
//! Converts an [`OdfParagraph`] and its inline children (spans, hyperlinks,
//! notes, fields, frames, comments) into the format-neutral [`Block`] / [`Inline`]
//! representation.

use loki_doc_model::content::annotation::{Comment, CommentRef, CommentRefKind};
use loki_doc_model::content::attr::{ExtensionBag, NodeAttr};
use loki_doc_model::content::block::{Block, StyledParagraph};
use loki_doc_model::content::field::types::{CrossRefFormat, Field, FieldKind};
use loki_doc_model::content::inline::{BookmarkKind, Inline, LinkTarget, NoteKind, StyledRun};
use loki_doc_model::style::catalog::StyleId;
use loki_doc_model::style::props::char_props::CharProps;
use loki_doc_model::style::props::revision::{RevisionKind, RevisionMark};

use crate::limits::MAX_REPEATED_SPACES;
use crate::odt::model::fields::OdfField;
use crate::odt::model::notes::{OdfNote, OdfNoteClass};
use crate::odt::model::paragraph::{OdfHyperlink, OdfParagraph, OdfParagraphChild, OdfSpan};
use crate::odt::model::revision::{OdfChangeKind, OdfChangedRegion};

use super::OdfMappingContext;
use super::frames::map_frame;
use super::meta::parse_datetime;

// ── Paragraphs ─────────────────────────────────────────────────────────────────

/// Convert an [`OdfParagraph`] to either [`Block::Heading`] (when
/// `is_heading` and `emit_heading_blocks` are both true) or
/// [`Block::StyledPara`].
pub(super) fn map_paragraph(para: &OdfParagraph, ctx: &mut OdfMappingContext<'_>) -> Block {
    let inlines = map_inline_children(&para.children, ctx);

    if para.is_heading && ctx.options.emit_heading_blocks {
        let level = para.outline_level.unwrap_or(1).clamp(1, 6);
        // Store the ODF style name in NodeAttr so the layout engine can look up
        // heading style properties from the catalog. Without this, the flow engine
        // falls back to hardcoded "Heading1"/"Heading2" IDs which don't match ODF
        // names like "Heading_20_1" (LibreOffice-encoded space).
        let mut attr = NodeAttr::default();
        if let Some(ref sn) = para.style_name {
            attr.kv.push(("style".to_string(), sn.clone()));
        }
        Block::Heading(level, attr, inlines)
    } else {
        let style_id = para.style_name.as_deref().map(StyleId::new);
        Block::StyledPara(StyledParagraph {
            style_id,
            direct_para_props: None,
            direct_char_props: None,
            inlines,
            attr: NodeAttr::default(),
        })
    }
}

// ── Inlines ────────────────────────────────────────────────────────────────────

pub(super) fn map_inline_children(
    children: &[OdfParagraphChild],
    ctx: &mut OdfMappingContext<'_>,
) -> Vec<Inline> {
    let mut out = Vec::new();
    // The insertion mark in force between a `change-start` and its `change-end`.
    let mut active: Option<RevisionMark> = None;
    for child in children {
        match child {
            OdfParagraphChild::RevisionStart { change_id } => {
                active = ctx.changed_regions.get(change_id).map(region_mark);
            }
            OdfParagraphChild::RevisionEnd { .. } => active = None,
            // A deletion point: re-materialise the removed text (kept in the
            // changed-region table) as a struck run so it renders and re-exports.
            OdfParagraphChild::RevisionPoint { change_id } => {
                if let Some(region) = ctx.changed_regions.get(change_id) {
                    out.push(deletion_run(region));
                }
            }
            _ => {
                if let Some(inl) = map_inline(child, ctx) {
                    out.push(match &active {
                        Some(mark) => wrap_revision(inl, mark),
                        None => inl,
                    });
                }
            }
        }
    }
    out
}

/// Builds a [`RevisionMark`] from a parsed changed-region (author/date verbatim,
/// so the RFC-3339 text round-trips exactly).
fn region_mark(region: &OdfChangedRegion) -> RevisionMark {
    RevisionMark {
        kind: match region.kind {
            OdfChangeKind::Insertion => RevisionKind::Insertion,
            OdfChangeKind::Deletion => RevisionKind::Deletion,
        },
        author: region.creator.clone(),
        date: region.date.clone(),
        id: Some(region.change_id.clone()),
    }
}

/// Wraps an inline in the given revision mark, folding it onto an existing
/// styled run's direct props or a fresh single-child run otherwise.
fn wrap_revision(inl: Inline, mark: &RevisionMark) -> Inline {
    match inl {
        Inline::StyledRun(mut sr) => {
            let mut cp = sr.direct_props.map(|b| *b).unwrap_or_default();
            cp.revision = Some(mark.clone());
            sr.direct_props = Some(Box::new(cp));
            Inline::StyledRun(sr)
        }
        other => Inline::StyledRun(revision_run(mark.clone(), vec![other])),
    }
}

/// Builds the struck run standing in for a tracked deletion's removed text.
fn deletion_run(region: &OdfChangedRegion) -> Inline {
    Inline::StyledRun(revision_run(
        region_mark(region),
        vec![Inline::Str(region.deleted_text.clone())],
    ))
}

/// A `StyledRun` carrying only a revision mark over `content`.
fn revision_run(mark: RevisionMark, content: Vec<Inline>) -> StyledRun {
    StyledRun {
        style_id: None,
        direct_props: Some(Box::new(CharProps {
            revision: Some(mark),
            ..CharProps::default()
        })),
        content,
        attr: NodeAttr::default(),
    }
}

fn map_inline(child: &OdfParagraphChild, ctx: &mut OdfMappingContext<'_>) -> Option<Inline> {
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
        // Tracked-change milestones are consumed by `map_inline_children`.
        OdfParagraphChild::SoftReturn
        | OdfParagraphChild::Other
        | OdfParagraphChild::RevisionStart { .. }
        | OdfParagraphChild::RevisionEnd { .. }
        | OdfParagraphChild::RevisionPoint { .. } => None,
        OdfParagraphChild::Tab => Some(Inline::Str("\t".into())),
        OdfParagraphChild::Space { count } => {
            // Clamp attacker-controlled <text:s text:c="N"/> counts so a tiny
            // element cannot force a multi-gigabyte allocation.
            Some(Inline::Str(
                " ".repeat((*count).min(MAX_REPEATED_SPACES) as usize),
            ))
        }
        OdfParagraphChild::LineBreak => Some(Inline::LineBreak),
        OdfParagraphChild::Annotation {
            name,
            creator,
            date,
            body,
        } => {
            let id = name.clone().unwrap_or_default();
            let mut comment = Comment::new(id.clone());
            comment.author.clone_from(creator);
            comment.date = date.as_deref().and_then(parse_datetime);
            comment.body = body
                .iter()
                .map(|t| Block::Para(vec![Inline::Str(t.clone())]))
                .collect();
            ctx.comments.push(comment);
            Some(Inline::Comment(CommentRef::new(id, CommentRefKind::Start)))
        }
        OdfParagraphChild::AnnotationEnd { name } => Some(Inline::Comment(CommentRef::new(
            name.clone().unwrap_or_default(),
            CommentRefKind::End,
        ))),
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
    let body: Vec<Block> = note
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

// ── Fields ─────────────────────────────────────────────────────────────────────

fn map_field(odf: &OdfField) -> Field {
    let kind = match odf {
        OdfField::PageNumber { .. } => FieldKind::PageNumber,
        OdfField::PageCount => FieldKind::PageCount,
        OdfField::Date { data_style, .. } => FieldKind::Date {
            format: data_style.clone(),
        },
        OdfField::Time { data_style, .. } => FieldKind::Time {
            format: data_style.clone(),
        },
        OdfField::Title => FieldKind::Title,
        OdfField::Subject => FieldKind::Subject,
        OdfField::AuthorName => FieldKind::Author,
        OdfField::FileName { .. } => FieldKind::FileName,
        OdfField::WordCount => FieldKind::WordCount,
        OdfField::CrossReference { ref_name, display } => {
            let format = match display.as_deref() {
                Some("number") => CrossRefFormat::Number,
                Some("page") => CrossRefFormat::Page,
                Some("caption") => CrossRefFormat::Caption,
                _ => CrossRefFormat::HeadingText,
            };
            FieldKind::CrossReference {
                target: ref_name.clone(),
                format,
            }
        }
        OdfField::ChapterName { display_levels } => FieldKind::Raw {
            instruction: format!("chapter display-levels={display_levels}"),
        },
        OdfField::Unknown { local_name, .. } => FieldKind::Raw {
            instruction: local_name.clone(),
        },
    };
    Field {
        kind,
        current_value: None,
        extensions: ExtensionBag::default(),
    }
}
