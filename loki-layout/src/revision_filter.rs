// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 AppThere Loki contributors

//! Non-destructive tracked-change **display** filtering.
//!
//! [`display_inlines`] rewrites a paragraph's inline list for the chosen
//! [`RevisionDisplay`] mode *without touching the document*: in
//! [`RevisionDisplay::Final`] the deletion runs are dropped and the insertion
//! runs keep their text but lose their revision mark (so they render as normal,
//! un-coloured text); [`RevisionDisplay::Original`] is the mirror. In
//! [`RevisionDisplay::AllMarkup`] (and whenever the paragraph has no tracked
//! runs) the input is returned borrowed — zero allocation on the common path.
//!
//! A tracked run is a `StyledRun` whose `direct_props.revision` is set — the
//! shape the DOCX/ODT importers and the editor produce for `w:ins`/`w:del`.

use std::borrow::Cow;

use loki_doc_model::content::inline::{Inline, StyledRun};
use loki_doc_model::style::props::revision::RevisionKind;

use crate::options::RevisionDisplay;

/// The revision kind carried directly on a run, if any.
fn run_revision_kind(run: &StyledRun) -> Option<RevisionKind> {
    run.direct_props
        .as_deref()
        .and_then(|p| p.revision.as_ref())
        .map(|r| r.kind)
}

/// Whether `inlines` contains any tracked run at the top level (cheap guard so
/// [`display_inlines`] can return `Borrowed` for the overwhelmingly common
/// no-revision paragraph).
fn has_tracked_run(inlines: &[Inline]) -> bool {
    inlines.iter().any(|i| match i {
        Inline::StyledRun(run) => run_revision_kind(run).is_some(),
        _ => false,
    })
}

/// Rewrite `inlines` for the tracked-change display `mode` (see the module doc).
///
/// Only the paragraph's **top-level** runs are filtered — the shape the
/// importers/editor produce (each `w:ins`/`w:del` is a top-level `StyledRun`).
/// A revision nested inside another inline wrapper is left as-is (it still
/// renders as markup); this matches how such content is authored in practice.
pub(crate) fn display_inlines(inlines: &[Inline], mode: RevisionDisplay) -> Cow<'_, [Inline]> {
    if mode == RevisionDisplay::AllMarkup || !has_tracked_run(inlines) {
        return Cow::Borrowed(inlines);
    }
    let mut out: Vec<Inline> = Vec::with_capacity(inlines.len());
    for inl in inlines {
        let Inline::StyledRun(run) = inl else {
            out.push(inl.clone());
            continue;
        };
        match (run_revision_kind(run), mode) {
            // Hidden in this view (accepted deletion / rejected insertion).
            (Some(RevisionKind::Deletion), RevisionDisplay::Final)
            | (Some(RevisionKind::Insertion), RevisionDisplay::Original) => {}
            // Shown as normal text: keep the run but strip its revision so
            // `revision_style::apply` adds no colour/underline/strikethrough.
            (Some(RevisionKind::Insertion), RevisionDisplay::Final)
            | (Some(RevisionKind::Deletion), RevisionDisplay::Original) => {
                let mut cleared = run.clone();
                if let Some(props) = cleared.direct_props.as_deref_mut() {
                    props.revision = None;
                }
                out.push(Inline::StyledRun(cleared));
            }
            // Untracked run, or a mode that keeps it as-is: clone unchanged.
            _ => out.push(inl.clone()),
        }
    }
    Cow::Owned(out)
}

#[cfg(test)]
#[path = "revision_filter_tests.rs"]
mod tests;
