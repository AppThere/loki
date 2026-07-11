// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 AppThere Loki contributors

//! Inline content mapper: paragraph children → [`Vec<Inline>`].
//!
//! Implements the OOXML complex-field state machine
//! (`w:fldChar` / `w:instrText`) to assemble [`Inline::Field`] values
//! and maps runs, hyperlinks, bookmarks, and drawings.

use loki_doc_model::content::annotation::{CommentRef, CommentRefKind};
use loki_doc_model::content::attr::NodeAttr;
use loki_doc_model::content::block::Block;
use loki_doc_model::content::field::types::Field;
use loki_doc_model::content::inline::{
    BookmarkKind, Inline, LinkTarget, MathType, NoteKind, StyledRun,
};
use loki_doc_model::style::catalog::StyleId;
use loki_doc_model::style::props::char_props::CharProps;
use loki_doc_model::style::props::revision::{RevisionKind, RevisionMark};

use crate::docx::model::paragraph::{
    DocxParaChild, DocxRevisionInfo, DocxRun, DocxRunChild, DocxTrackedChange,
};
use crate::error::{NoteKind as WarnNoteKind, OoxmlWarning};

use super::document::MappingContext;
use super::fields::{fld_simple_text, parse_field_instruction};
use super::images::map_drawing;
use super::props::map_rpr;

// ── Field state machine ────────────────────────────────────────────────────────

/// Assembly state for OOXML complex fields (`w:fldChar`/`w:instrText`).
///
/// `depth` tracks nested `begin` elements so that fields embedded inside
/// other field instructions are handled without panicking.
enum FieldState {
    Normal,
    /// Accumulating field instruction text. `depth ≥ 1`.
    InCode {
        instruction: String,
        depth: usize,
    },
    /// After `w:fldChar @w:fldCharType="separate"`: accumulating snapshot text.
    InResult {
        instruction: String,
        snapshot: String,
        depth: usize,
    },
}

// ── Public entry point ─────────────────────────────────────────────────────────

/// Maps a paragraph's children into a sequence of [`Inline`]s.
///
/// Implements the OOXML complex-field state machine across all children so
/// that fields that span multiple runs are assembled correctly.
pub(crate) fn map_inlines(children: &[DocxParaChild], ctx: &mut MappingContext<'_>) -> Vec<Inline> {
    let mut result: Vec<Inline> = Vec::new();
    let mut state = FieldState::Normal;

    for child in children {
        match child {
            DocxParaChild::Run(run) => {
                result.extend(process_run(run, &mut state, ctx, None));
            }
            DocxParaChild::Hyperlink(h) => {
                let url = if let Some(rel_id) = &h.rel_id {
                    if let Some(target) = ctx.hyperlinks.get(rel_id) {
                        target.clone()
                    } else {
                        ctx.warnings.push(OoxmlWarning::UnresolvedRelationship {
                            id: rel_id.clone(),
                            context: "hyperlink".to_string(),
                        });
                        format!("#{rel_id}")
                    }
                } else if let Some(anchor) = &h.anchor {
                    format!("#{anchor}")
                } else {
                    String::new()
                };
                let inner: Vec<Inline> = h
                    .runs
                    .iter()
                    .flat_map(|r| process_run_simple(r, ctx))
                    .collect();
                result.push(Inline::Link(
                    NodeAttr::default(),
                    inner,
                    LinkTarget { url, title: None },
                ));
            }
            DocxParaChild::BookmarkStart { id, name } => {
                // COMPAT(microsoft): w:bookmarkStart/End IDs must be unique per
                // OOXML §17.13.6.2, but programmatically generated documents
                // frequently use duplicate IDs (e.g. all bookmarks with id="1").
                // We handle this gracefully by tracking open bookmarks in a LIFO
                // stack within the MappingContext, popping the most recent matching
                // ID to resolve the bookmark name at BookmarkEnd.
                ctx.open_bookmarks.push((id.clone(), name.clone()));
                result.push(Inline::Bookmark(BookmarkKind::Start, name.clone()));
            }
            DocxParaChild::BookmarkEnd { id } => {
                let name = if let Some(pos) = ctx
                    .open_bookmarks
                    .iter()
                    .rposition(|(open_id, _)| open_id == id)
                {
                    let (_, name) = ctx.open_bookmarks.remove(pos);
                    name
                } else {
                    id.clone()
                };
                result.push(Inline::Bookmark(BookmarkKind::End, name));
            }
            DocxParaChild::TrackDel(change) => {
                extend_tracked(&mut result, change, RevisionKind::Deletion, &mut state, ctx);
            }
            DocxParaChild::TrackIns(change) => {
                extend_tracked(
                    &mut result,
                    change,
                    RevisionKind::Insertion,
                    &mut state,
                    ctx,
                );
            }
            DocxParaChild::CommentRangeStart { id } => {
                result.push(Inline::Comment(CommentRef::new(
                    id.clone(),
                    CommentRefKind::Start,
                )));
            }
            DocxParaChild::CommentRangeEnd { id } => {
                result.push(Inline::Comment(CommentRef::new(
                    id.clone(),
                    CommentRefKind::End,
                )));
            }
            DocxParaChild::Math { mathml, display } => {
                let kind = if *display {
                    MathType::DisplayMath
                } else {
                    MathType::InlineMath
                };
                result.push(Inline::Math(kind, mathml.clone()));
            }
            DocxParaChild::SimpleField { instr, runs } => {
                // `w:fldSimple` is a self-contained field: the `@w:instr`
                // instruction plus the cached result as child runs. Map it the
                // same way a complex field is, so both forms produce an
                // identical `Inline::Field`.
                let kind = parse_field_instruction(instr);
                let snapshot = fld_simple_text(runs);
                let mut field = Field::new(kind);
                let trimmed = snapshot.trim();
                if !trimmed.is_empty() {
                    field.current_value = Some(trimmed.to_string());
                }
                result.push(Inline::Field(field));
            }
        }
    }

    result
}

// ── Run processing ─────────────────────────────────────────────────────────────

/// Processes a single run through the field state machine.
///
/// Returns the resulting inlines, wrapped in a [`StyledRun`] when the run
/// has an explicit character style or direct formatting properties.
/// Maps a tracked change's runs (a `w:ins`/`w:del`), attaching a [`RevisionMark`]
/// of `kind` (with the change's author/date/id) to every run so it round-trips.
fn extend_tracked(
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

fn process_run(
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
fn process_run_simple(run: &DocxRun, ctx: &mut MappingContext<'_>) -> Vec<Inline> {
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

// ── Field state transitions ────────────────────────────────────────────────────

fn process_fld_char(
    fld_char_type: &str,
    state: &mut FieldState,
    raw: &mut Vec<Inline>,
    ctx: &mut MappingContext<'_>,
) {
    match fld_char_type {
        "begin" => {
            let new_state = match &*state {
                FieldState::Normal => FieldState::InCode {
                    instruction: String::new(),
                    depth: 1,
                },
                FieldState::InCode { instruction, depth } => FieldState::InCode {
                    instruction: instruction.clone(),
                    depth: depth + 1,
                },
                FieldState::InResult {
                    instruction,
                    snapshot,
                    depth,
                } => FieldState::InResult {
                    instruction: instruction.clone(),
                    snapshot: snapshot.clone(),
                    depth: depth + 1,
                },
            };
            *state = new_state;
        }
        "separate" => {
            // Only transition at depth == 1; deeper separates belong to nested fields.
            if let FieldState::InCode { instruction, depth } = &*state
                && *depth == 1
            {
                let new_state = FieldState::InResult {
                    instruction: instruction.clone(),
                    snapshot: String::new(),
                    depth: 1,
                };
                *state = new_state;
            }
        }
        "end" => {
            match state {
                FieldState::InCode { depth, .. } | FieldState::InResult { depth, .. }
                    if *depth > 1 =>
                {
                    *depth -= 1;
                }
                FieldState::InCode { instruction, .. } => {
                    let kind = parse_field_instruction(instruction);
                    raw.push(Inline::Field(Field::new(kind)));
                    *state = FieldState::Normal;
                }
                FieldState::InResult {
                    instruction,
                    snapshot,
                    ..
                } => {
                    let kind = parse_field_instruction(instruction);
                    let cv = snapshot.trim().to_string();
                    let mut field = Field::new(kind);
                    if !cv.is_empty() {
                        field.current_value = Some(cv);
                    }
                    raw.push(Inline::Field(field));
                    *state = FieldState::Normal;
                }
                FieldState::Normal => {
                    // Malformed: `end` with no matching `begin`.
                    ctx.warnings.push(OoxmlWarning::UnrecognisedField {
                        instruction: "<malformed:end-without-begin>".to_string(),
                    });
                }
            }
        }
        _ => {} // Unknown fldCharType; ignore.
    }
}

// ── Note lookup helper ─────────────────────────────────────────────────────────

fn lookup_note(
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

// ── Tests ─────────────────────────────────────────────────────────────────────

#[cfg(test)]
#[path = "inline_tests.rs"]
mod tests;
