// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 AppThere Loki contributors

//! Inline content mapper: paragraph children → [`Vec<Inline>`].
//!
//! Implements the OOXML complex-field state machine
//! (`w:fldChar` / `w:instrText`) to assemble [`Inline::Field`] values
//! and maps runs, hyperlinks, bookmarks, and drawings.

mod field_state;
mod run;

#[cfg(test)]
mod tests_field;
#[cfg(test)]
mod tests_run;

use loki_doc_model::content::attr::NodeAttr;
use loki_doc_model::content::inline::{BookmarkKind, Inline, LinkTarget};

use crate::docx::model::paragraph::DocxParaChild;
use crate::error::OoxmlWarning;

use super::document::MappingContext;
use field_state::FieldState;
use run::{process_run, process_run_simple};

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
                result.extend(process_run(run, &mut state, ctx));
            }
            DocxParaChild::Hyperlink(h) => {
                let url = if let Some(rel_id) = &h.rel_id {
                    if let Some(target) = ctx.hyperlinks.get(rel_id) {
                        target.clone() as String
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
            DocxParaChild::TrackDel(_) => {
                // Deleted content is skipped; it is no longer part of the document.
            }
            DocxParaChild::TrackIns(runs) => {
                // Accepted insertions are treated as normal runs.
                for run in runs {
                    result.extend(process_run(run, &mut state, ctx));
                }
            }
        }
    }

    result
}
