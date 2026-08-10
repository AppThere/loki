// SPDX-License-Identifier: Apache-2.0

//! Link target kinds and the in-document outline the picker lists (design notes
//! 19 and 20).

use loki_doc_model::Document;
use loki_doc_model::content::block::Block;
use loki_doc_model::content::inline::{BookmarkKind, Inline};
use loki_i18n::fl;

use super::super::dialog_walk::{block_inlines, visit_doc_blocks};

/// The four kinds of thing a link can point at.
///
/// A segmented control rather than tabs: the kinds are **mutually exclusive
/// alternatives**, not facets of one object, and picking one replaces the body
/// rather than revealing more of the same form (design note 19).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(in crate::routes::editor) enum LinkKind {
    /// An absolute `http`/`https` URL.
    Web,
    /// A heading or bookmark inside this document.
    Document,
    /// A `mailto:` address.
    Email,
    /// A `file:` path.
    File,
}

impl LinkKind {
    /// Every kind, in display order.
    pub const ALL: [LinkKind; 4] = [
        LinkKind::Web,
        LinkKind::Document,
        LinkKind::Email,
        LinkKind::File,
    ];

    /// The localized segment label. Compact abbreviates `Document` to `Doc` —
    /// four segments have to fit a phone's width, and the other three are
    /// already short.
    #[must_use]
    pub fn label(self, compact: bool) -> String {
        match self {
            LinkKind::Web => fl!("link-dialog-kind-web"),
            LinkKind::Document if compact => fl!("link-dialog-kind-document-short"),
            LinkKind::Document => fl!("link-dialog-kind-document"),
            LinkKind::Email => fl!("link-dialog-kind-email"),
            LinkKind::File => fl!("link-dialog-kind-file"),
        }
    }

    /// The kind at `index`, saturating at the last kind.
    #[must_use]
    pub fn from_index(index: usize) -> Self {
        LinkKind::ALL.get(index).copied().unwrap_or(LinkKind::File)
    }

    /// This kind's position in the control.
    #[must_use]
    pub fn index(self) -> usize {
        LinkKind::ALL.iter().position(|k| *k == self).unwrap_or(0)
    }
}

/// What a target row in the picker points at.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum TargetKind {
    /// A heading, at the given outline level (1–6).
    Heading(u8),
    /// A named bookmark.
    Bookmark,
}

/// One row in the in-document target picker.
///
/// Named `OutlineTarget` rather than `LinkTarget`: the model already has a
/// [`loki_doc_model::content::inline::LinkTarget`], and two types with one name
/// in the same feature is how the wrong one gets imported.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(super) struct OutlineTarget {
    /// The anchor written into the URL fragment.
    pub anchor: String,
    /// The label shown in the list — heading text, or the bookmark's name.
    pub label: String,
    /// Which kind of anchor this is.
    pub kind: TargetKind,
}

impl OutlineTarget {
    /// The `#anchor` this row inserts.
    #[must_use]
    pub fn href(&self) -> String {
        format!("#{}", self.anchor)
    }

    /// The indent depth of the row — headings nest, bookmarks never do.
    #[must_use]
    pub fn depth(&self) -> u8 {
        match self.kind {
            TargetKind::Heading(level) => level.saturating_sub(1).min(5),
            TargetKind::Bookmark => 0,
        }
    }
}

/// Every heading and bookmark in `doc`, in document order.
///
/// # Picked from the outline, never typed
///
/// In-document targets are chosen from the live document (design note 20). A
/// typed anchor is a link that silently breaks the moment a heading is renamed,
/// and nothing in the UI would show it had.
#[must_use]
pub(super) fn document_targets(doc: &Document) -> Vec<OutlineTarget> {
    // The shared walk (`dialog_walk`), so an anchor in a table's header row is
    // offered like any other. Note 20 forbids typing an anchor by hand, so a
    // target this list omits cannot be linked to at all.
    let mut out = Vec::new();
    visit_doc_blocks(doc, &mut |block| {
        if let Block::Heading(level, attr, inlines) = block {
            let label = plain_text(inlines);
            // A heading with no text cannot be told from its neighbours in the
            // list, so it is not offered as a target.
            if !label.trim().is_empty() {
                let anchor = attr
                    .id
                    .clone()
                    .unwrap_or_else(|| slugify(&label, out.len()));
                out.push(OutlineTarget {
                    anchor,
                    label,
                    kind: TargetKind::Heading((*level).clamp(1, 6)),
                });
            }
        }
        for inlines in block_inlines(block) {
            collect_inlines(inlines, &mut out);
        }
    });
    out
}

/// Collects bookmark starts from an inline sequence.
///
/// Only `Start` markers become targets: a range bookmark emits both, and
/// offering the end as a separate destination would put two identical names in
/// the list pointing at opposite ends of the same span.
fn collect_inlines(inlines: &[Inline], out: &mut Vec<OutlineTarget>) {
    for inline in inlines {
        match inline {
            Inline::Bookmark(BookmarkKind::Start, name) if !name.trim().is_empty() => {
                out.push(OutlineTarget {
                    anchor: name.clone(),
                    label: name.clone(),
                    kind: TargetKind::Bookmark,
                });
            }
            Inline::StyledRun(run) => collect_inlines(&run.content, out),
            Inline::Span(_, inner)
            | Inline::Emph(inner)
            | Inline::Strong(inner)
            | Inline::Underline(inner)
            | Inline::Strikeout(inner)
            | Inline::Superscript(inner)
            | Inline::Subscript(inner)
            | Inline::SmallCaps(inner) => collect_inlines(inner, out),
            Inline::Link(_, inner, _) => collect_inlines(inner, out),
            _ => {}
        }
    }
}

/// The concatenated plain text of an inline sequence.
/// Collapses runs of separators down to one.
///
/// `replace("--", "-")` is a single non-overlapping pass, so three or more in a
/// row survived it: "One \u{b7} The harbour" slugged to `one--the-harbour`.
fn collapse_dashes(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    let mut last_dash = false;
    for c in s.chars() {
        if c == '-' {
            if !last_dash {
                out.push(c);
            }
            last_dash = true;
        } else {
            out.push(c);
            last_dash = false;
        }
    }
    out
}

fn plain_text(inlines: &[Inline]) -> String {
    let mut s = String::new();
    push_plain(inlines, &mut s);
    s.trim().to_string()
}

fn push_plain(inlines: &[Inline], out: &mut String) {
    for inline in inlines {
        match inline {
            Inline::Str(t) => out.push_str(t),
            Inline::Space | Inline::SoftBreak | Inline::LineBreak => out.push(' '),
            Inline::StyledRun(run) => push_plain(&run.content, out),
            Inline::Span(_, inner)
            | Inline::Emph(inner)
            | Inline::Strong(inner)
            | Inline::Underline(inner)
            | Inline::Strikeout(inner)
            | Inline::Superscript(inner)
            | Inline::Subscript(inner)
            | Inline::SmallCaps(inner) => push_plain(inner, out),
            Inline::Link(_, inner, _) => push_plain(inner, out),
            _ => {}
        }
    }
}

/// A stable anchor for a heading that carries no id of its own.
///
/// `ordinal` disambiguates: two chapters called "Introduction" would otherwise
/// share an anchor and the second link would jump to the first.
fn slugify(label: &str, ordinal: usize) -> String {
    let base: String = label
        .chars()
        .map(|c| {
            if c.is_ascii_alphanumeric() {
                c.to_ascii_lowercase()
            } else {
                '-'
            }
        })
        .collect();
    let base = collapse_dashes(base.trim_matches('-'));
    if base.is_empty() {
        format!("heading-{ordinal}")
    } else {
        format!("{base}-{ordinal}")
    }
}

#[cfg(test)]
#[path = "target_tests.rs"]
mod tests;
