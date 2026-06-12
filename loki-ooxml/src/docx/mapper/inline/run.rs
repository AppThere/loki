// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 AppThere Loki contributors

//! Run processing: maps [`DocxRun`] and [`DocxRunChild`] into [`Inline`]s.

use loki_doc_model::content::block::Block;
use loki_doc_model::content::attr::NodeAttr;
use loki_doc_model::content::inline::{Inline, NoteKind, StyledRun};
use loki_doc_model::style::catalog::StyleId;
use loki_doc_model::style::props::char_props::CharProps;

use crate::docx::model::paragraph::{DocxRun, DocxRunChild};
use crate::error::{NoteKind as WarnNoteKind, OoxmlWarning};

use super::super::document::MappingContext;
use super::super::images::map_drawing;
use super::super::props::map_rpr;
use super::field_state::{process_fld_char, FieldState};

// ── Run processing ─────────────────────────────────────────────────────────────

/// Processes a single run through the field state machine.
///
/// Returns the resulting inlines, wrapped in a [`StyledRun`] when the run
/// has an explicit character style or direct formatting properties.
pub(super) fn process_run(
    run: &DocxRun,
    state: &mut FieldState,
    ctx: &mut MappingContext<'_>,
) -> Vec<Inline> {
    let char_props = run.rpr.as_ref().map(map_rpr);
    let style_id = run
        .rpr
        .as_ref()
        .and_then(|rpr| rpr.style_id.as_ref())
        .map(|s| StyleId::new(s.clone()));

    let mut raw: Vec<Inline> = Vec::new();

    for child in &run.children {
        process_run_child(child, state, &mut raw, ctx);
    }

    // Wrap in StyledRun only when there is explicit formatting to preserve.
    let needs_wrap = style_id.is_some()
        || char_props
            .as_ref()
            .is_some_and(|p| p != &CharProps::default());

    if needs_wrap && !raw.is_empty() {
        let styled = StyledRun {
            style_id,
            direct_props: char_props.map(Box::new),
            content: raw,
            attr: NodeAttr::default(),
        };
        vec![Inline::StyledRun(styled)]
    } else {
        raw
    }
}

/// Maps the runs in a hyperlink or similar context, using a fresh field state.
pub(super) fn process_run_simple(run: &DocxRun, ctx: &mut MappingContext<'_>) -> Vec<Inline> {
    let mut state = FieldState::Normal;
    process_run(run, &mut state, ctx)
}

// ── Run child dispatch ─────────────────────────────────────────────────────────

pub(super) fn process_run_child(
    child: &DocxRunChild,
    state: &mut FieldState,
    raw: &mut Vec<Inline>,
    ctx: &mut MappingContext<'_>,
) {
    match child {
        DocxRunChild::FldChar { fld_char_type } => {
            process_fld_char(fld_char_type, state, raw, &mut ctx.warnings);
        }
        DocxRunChild::InstrText { text } => {
            if let FieldState::InCode { instruction, depth } = state
                && *depth == 1
            {
                instruction.push_str(text);
            }
            // depth > 1: instruction text for an inner nested field; ignored.
        }
        DocxRunChild::Text { text, .. } => match state {
            FieldState::Normal => raw.push(Inline::Str(text.clone())),
            FieldState::InResult {
                snapshot, depth, ..
            } if *depth == 1 => {
                snapshot.push_str(text);
            }
            _ => {} // inside field code or nested field
        },
        DocxRunChild::Tab => {
            if matches!(state, FieldState::Normal) {
                raw.push(Inline::Str("\t".to_string()));
            }
        }
        DocxRunChild::Break { break_type } => {
            if matches!(state, FieldState::Normal) {
                match break_type.as_deref() {
                    // Page and column breaks are promoted to paragraph-level flags
                    // (page_break_after / column_break_after) in map_paragraph.
                    Some("page" | "column") => {}
                    _ => raw.push(Inline::LineBreak),
                }
            }
        }
        DocxRunChild::FootnoteRef { id } => {
            if matches!(state, FieldState::Normal) {
                let blocks = lookup_note(
                    ctx.footnotes,
                    *id,
                    &mut ctx.warnings,
                    WarnNoteKind::Footnote,
                );
                raw.push(Inline::Note(NoteKind::Footnote, blocks));
            }
        }
        DocxRunChild::EndnoteRef { id } => {
            if matches!(state, FieldState::Normal) {
                let blocks =
                    lookup_note(ctx.endnotes, *id, &mut ctx.warnings, WarnNoteKind::Endnote);
                raw.push(Inline::Note(NoteKind::Endnote, blocks));
            }
        }
        DocxRunChild::Drawing(drawing) => {
            if matches!(state, FieldState::Normal)
                && let Some(img) = map_drawing(drawing, ctx)
            {
                raw.push(img);
            }
        }
    }
}

// ── Note lookup helper ─────────────────────────────────────────────────────────

pub(super) fn lookup_note(
    map: &std::collections::HashMap<i32, Vec<Block>>,
    id: i32,
    warnings: &mut Vec<OoxmlWarning>,
    kind: WarnNoteKind,
) -> Vec<Block> {
    map.get(&id).cloned().unwrap_or_else(|| {
        warnings.push(OoxmlWarning::MissingNoteContent { id, kind });
        Vec::new()
    })
}
