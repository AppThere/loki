// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 AppThere Loki contributors

//! Tracked-change (revision) mapping helpers for [`super`] (`inlines.rs`):
//! changed-region → [`RevisionMark`] conversion, run wrapping, deleted-text
//! re-materialisation, and ¶-mark deletion detection. Split out to hold the
//! file-size ceiling (cohesive-cluster extraction, Phase 7.1 technique 3).

use loki_doc_model::content::attr::NodeAttr;
use loki_doc_model::content::inline::{Inline, StyledRun};
use loki_doc_model::style::props::char_props::CharProps;
use loki_doc_model::style::props::revision::{RevisionKind, RevisionMark};

use crate::odt::model::paragraph::{OdfParagraph, OdfParagraphChild};
use crate::odt::model::revision::{OdfChangeKind, OdfChangedRegion};

use super::super::OdfMappingContext;

/// Builds a [`RevisionMark`] from a parsed changed-region (author/date verbatim,
/// so the RFC-3339 text round-trips exactly).
pub(super) fn region_mark(region: &OdfChangedRegion) -> RevisionMark {
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
pub(super) fn wrap_revision(inl: Inline, mark: &RevisionMark) -> Inline {
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
pub(super) fn deletion_run(region: &OdfChangedRegion) -> Inline {
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

/// The ¶-mark deletion recorded in this paragraph, if any: a `text:change`
/// point whose deletion region stows **no** text — the deleted paragraph
/// break itself (the shape the ODT writer's `para_mark_change` emits). A
/// point with removed text is a run deletion, not a ¶ deletion.
pub(super) fn para_mark_revision(
    para: &OdfParagraph,
    ctx: &OdfMappingContext<'_>,
) -> Option<RevisionMark> {
    para.children.iter().find_map(|c| match c {
        OdfParagraphChild::RevisionPoint { change_id } => ctx
            .changed_regions
            .get(change_id)
            .filter(|r| r.kind == OdfChangeKind::Deletion && r.deleted_text.is_empty())
            .map(region_mark),
        _ => None,
    })
}
