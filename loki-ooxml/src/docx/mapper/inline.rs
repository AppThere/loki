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
use loki_doc_model::content::inline::{BookmarkKind, Inline, LinkTarget, MathType};
use loki_doc_model::style::props::revision::RevisionKind;

use crate::docx::model::paragraph::DocxParaChild;
use crate::error::{NoteKind as WarnNoteKind, OoxmlWarning};

use super::document::MappingContext;
use super::fields::{fld_simple_text, parse_field_instruction};

#[path = "inline_run.rs"]
mod run;
use run::{extend_tracked, process_run, process_run_simple};

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

    // A complex field still open at the paragraph's end spans into the next
    // paragraph (e.g. a multi-entry TOC field, whose `begin`/`separate` sit in
    // the first entry's paragraph and whose `end` is in the last). Emit the
    // result text accumulated so far as plain inline content so this paragraph's
    // entry is not dropped; the continuation paragraphs render as normal text.
    if let FieldState::InResult { snapshot, .. } = state
        && !snapshot.trim().is_empty()
    {
        result.push(Inline::Str(snapshot));
    }

    result
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
