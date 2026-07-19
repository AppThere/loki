// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 AppThere Loki contributors

//! Run processing for the inline mapper (split from `inline.rs` for the
//! 300-line ceiling): maps a `w:r` (and tracked-change `w:ins`/`w:del` run
//! groups) through the complex-field state machine into `Inline`s, wrapping
//! in a `StyledRun` when the run carries a character style or direct
//! formatting. The `FieldState` enum and the `w:fldChar` transition handler
//! live in `inline.rs` and are reached via `super::`.

use loki_doc_model::content::attr::NodeAttr;
use loki_doc_model::content::inline::{Inline, NoteKind, StyledRun};
use loki_doc_model::style::catalog::StyleId;
use loki_doc_model::style::props::char_props::CharProps;
use loki_doc_model::style::props::revision::{RevisionKind, RevisionMark};

use super::{FieldState, lookup_note, process_fld_char};
use crate::docx::mapper::document::MappingContext;
use crate::docx::mapper::images::map_drawing;
use crate::docx::mapper::props::map_rpr;
use crate::docx::model::paragraph::{DocxRevisionInfo, DocxRun, DocxRunChild, DocxTrackedChange};
use crate::error::NoteKind as WarnNoteKind;

// ── Run processing ─────────────────────────────────────────────────────────────

/// Maps a tracked change's runs (a `w:ins`/`w:del`), attaching a [`RevisionMark`]
/// of `kind` (with the change's author/date/id) to every run so it round-trips.
pub(super) fn extend_tracked(
    out: &mut Vec<Inline>,
    change: &DocxTrackedChange,
    kind: RevisionKind,
    state: &mut FieldState,
    ctx: &mut MappingContext<'_>,
) {
    let DocxRevisionInfo { author, date, id } = &change.info;
    let rev = RevisionMark {
        kind,
        author: author.clone(),
        date: date.clone(),
        id: id.clone(),
    };
    for run in &change.runs {
        out.extend(process_run(run, state, ctx, Some(&rev)));
    }
}

/// Processes a single run through the field state machine.
///
/// Returns the resulting inlines, wrapped in a [`StyledRun`] when the run
/// has an explicit character style or direct formatting properties.
pub(super) fn process_run(
    run: &DocxRun,
    state: &mut FieldState,
    ctx: &mut MappingContext<'_>,
    revision: Option<&RevisionMark>,
) -> Vec<Inline> {
    let mut char_props = run.rpr.as_ref().map(map_rpr);
    if let Some(rev) = revision {
        char_props.get_or_insert_with(CharProps::default).revision = Some(rev.clone());
    }
    let style_id = run
        .rpr
        .as_ref()
        .and_then(|rpr| rpr.style_id.as_ref())
        .map(|s| StyleId::new(s.clone()));

    let mut raw: Vec<Inline> = Vec::new();

    for child in &run.children {
        process_run_child(child, state, &mut raw, ctx);
    }

    // Wrap in StyledRun to preserve explicit formatting or a tracked-change mark.
    let needs_wrap = style_id.is_some()
        || revision.is_some()
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
    process_run(run, &mut state, ctx, None)
}

// ── Run child dispatch ─────────────────────────────────────────────────────────

fn process_run_child(
    child: &DocxRunChild,
    state: &mut FieldState,
    raw: &mut Vec<Inline>,
    ctx: &mut MappingContext<'_>,
) {
    match child {
        DocxRunChild::FldChar { fld_char_type } => {
            process_fld_char(fld_char_type, state, raw, ctx);
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
        DocxRunChild::Tab => match state {
            FieldState::Normal => raw.push(Inline::Str("\t".to_string())),
            // A tab inside a field's cached result (e.g. a TOC entry's leader
            // tab between the heading text and its page number) is part of the
            // visible result — keep it in the snapshot so it survives.
            FieldState::InResult {
                snapshot, depth, ..
            } if *depth == 1 => snapshot.push('\t'),
            _ => {}
        },
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
