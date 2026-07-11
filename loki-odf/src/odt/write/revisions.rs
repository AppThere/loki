// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 AppThere Loki contributors

//! Tracked-change (revision) serialisation for `content.xml`. ODF 1.3 §5.5.
//!
//! A run whose [`CharProps::revision`] is set is emitted as an ODF change: an
//! **insertion** brackets its `text:span` with `text:change-start` /
//! `text:change-end`; a **deletion** emits a single `text:change` point and
//! stows the removed text in the `text:tracked-changes` region table (rendered
//! once at the start of `office:text`). The `office:change-info` carries
//! `dc:creator` / `dc:date`, mirroring [`super::inlines`]'s comment writer.

use loki_doc_model::content::block::StyledParagraph;
use loki_doc_model::content::inline::StyledRun;
use loki_doc_model::style::props::revision::{RevisionKind, RevisionMark};

use super::content::Cx;
use super::inlines::{plain_text, wrap_span};
use super::xml::{attr, escape};

/// One `text:changed-region` entry accumulated during body rendering.
struct Region {
    id: String,
    kind: RevisionKind,
    author: Option<String>,
    date: Option<String>,
    /// The removed text for a deletion; `None` for an insertion.
    deleted_text: Option<String>,
}

/// Collector for the document's tracked-change regions, assigning change ids and
/// rendering the `text:tracked-changes` table.
#[derive(Default)]
pub(super) struct Changes {
    regions: Vec<Region>,
    counter: usize,
}

impl Changes {
    /// Records a revision, returning its change id (the mark's own id when set,
    /// else a freshly generated `ct{n}`). `deleted_text` is `Some` for a
    /// deletion (the removed content stored in the region), `None` otherwise.
    fn register(&mut self, rev: &RevisionMark, deleted_text: Option<String>) -> String {
        self.counter += 1;
        let id = rev
            .id
            .clone()
            .unwrap_or_else(|| format!("ct{}", self.counter));
        self.regions.push(Region {
            id: id.clone(),
            kind: rev.kind,
            author: rev.author.clone(),
            date: rev.date.clone(),
            deleted_text,
        });
        id
    }

    /// Renders the `<text:tracked-changes>` table, or `""` when there are none.
    #[must_use]
    pub(super) fn render(&self) -> String {
        if self.regions.is_empty() {
            return String::new();
        }
        let mut out = String::from("<text:tracked-changes>");
        for r in &self.regions {
            out.push_str("<text:changed-region");
            attr(&mut out, "text:id", &r.id);
            out.push('>');
            let wrapper = match r.kind {
                RevisionKind::Insertion => "text:insertion",
                RevisionKind::Deletion => "text:deletion",
            };
            out.push_str(&format!("<{wrapper}>"));
            write_change_info(&mut out, r);
            if let Some(text) = &r.deleted_text {
                out.push_str("<text:p>");
                out.push_str(&escape(text));
                out.push_str("</text:p>");
            }
            out.push_str(&format!("</{wrapper}></text:changed-region>"));
        }
        out.push_str("</text:tracked-changes>");
        out
    }
}

/// Writes the `office:change-info` (creator + date) for a region.
fn write_change_info(out: &mut String, r: &Region) {
    out.push_str("<office:change-info>");
    if let Some(author) = &r.author {
        out.push_str(&format!("<dc:creator>{}</dc:creator>", escape(author)));
    }
    if let Some(date) = &r.date {
        out.push_str(&format!("<dc:date>{}</dc:date>", escape(date)));
    }
    out.push_str("</office:change-info>");
}

/// Writes an `Inline::StyledRun`, bracketing it as a tracked change when its
/// direct props carry a [`RevisionMark`], else as a plain styled span.
pub(super) fn write_styled_run(out: &mut String, sr: &StyledRun, cx: &mut Cx) {
    let rev = sr.direct_props.as_deref().and_then(|p| p.revision.clone());
    match rev {
        // Deletion: the text lives only in the region table; the body keeps a
        // single change point.
        Some(rev) if rev.kind == RevisionKind::Deletion => {
            let text = plain_text(&sr.content);
            let id = cx.changes.register(&rev, Some(text));
            out.push_str("<text:change");
            attr(out, "text:change-id", &id);
            out.push_str("/>");
        }
        // Insertion: the inserted span stays inline, bracketed by milestones.
        Some(rev) => {
            let id = cx.changes.register(&rev, None);
            out.push_str("<text:change-start");
            attr(out, "text:change-id", &id);
            out.push_str("/>");
            wrap_span(out, span_style(sr, cx).as_deref(), &sr.content, cx);
            out.push_str("<text:change-end");
            attr(out, "text:change-id", &id);
            out.push_str("/>");
        }
        None => wrap_span(out, span_style(sr, cx).as_deref(), &sr.content, cx),
    }
}

/// The end-of-paragraph `<text:change/>` milestone for a tracked ¶-mark
/// deletion on a styled paragraph's `direct_char_props`, or `""` when the
/// paragraph mark carries no tracked deletion. The registered region stows an
/// empty `<text:p/>` — the deleted paragraph break, LibreOffice's shape for a
/// ¶ deletion — which the importer maps back onto the paragraph.
pub(super) fn para_mark_change(sp: &StyledParagraph, cx: &mut Cx) -> String {
    let Some(rev) = sp
        .direct_char_props
        .as_deref()
        .and_then(|p| p.revision.as_ref())
        .filter(|r| r.kind == RevisionKind::Deletion)
    else {
        return String::new();
    };
    let id = cx.changes.register(rev, Some(String::new()));
    let mut out = String::from("<text:change");
    attr(&mut out, "text:change-id", &id);
    out.push_str("/>");
    out
}

/// Resolves the automatic text-style name for a styled run (its direct props, or
/// its named style). Revision-only props yield `None` (no formatting to emit).
fn span_style(sr: &StyledRun, cx: &mut Cx) -> Option<String> {
    match sr.direct_props.as_deref() {
        Some(dp) => cx.auto.text_style(dp),
        None => sr.style_id.as_ref().map(|s| s.as_str().to_string()),
    }
}
