// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 AppThere Loki contributors

//! Cross-part repair: drop footnote/endnote **separator references** in
//! `settings.xml` that have no backing notes part.
//!
//! A `<w:footnotePr>`/`<w:endnotePr>` block in `settings.xml` may carry
//! `<w:footnote w:id="…"/>`/`<w:endnote w:id="…"/>` children — references to the
//! special *separator* notes (ids `-1`/`0`) that live in `footnotes.xml` /
//! `endnotes.xml`. Word resolves each reference against that stream and reports
//! an error in "Footnotes"/"Endnotes" when the stream (or the referenced id) is
//! absent; a tolerant reader (Loki) ignores the dangling reference and opens the
//! file. Word's own output always pairs such a reference with a backing part.
//!
//! Unlike the ordering and `mc:Ignorable` passes this is a **cross-part** check:
//! the offending element is in `settings.xml`, but whether it is offending
//! depends on the *other* parts. [`NoteContext`] carries the ids each notes part
//! actually contains (or `None` when the part is absent).
//!
//! The fix is **lossless**: a reference that resolves to nothing conveys no
//! information, so removing the dangling `<w:footnote>`/`<w:endnote>` child —
//! leaving every other setting, and any reference that *does* resolve, intact —
//! changes nothing a consumer could have acted on. An emptied `<w:…Pr>` block
//! remains a valid empty element.

use std::collections::HashSet;

use super::RepairFinding;
use super::dom::{Elem, Node};

/// Which note stream a `<w:…Pr>` block concerns.
#[derive(Clone, Copy)]
enum Kind {
    Footnote,
    Endnote,
}

impl Kind {
    /// The separator-reference element name (`footnote`/`endnote`).
    fn note(self) -> &'static str {
        match self {
            Kind::Footnote => "footnote",
            Kind::Endnote => "endnote",
        }
    }
    /// The settings container name (`footnotePr`/`endnotePr`).
    fn pr(self) -> &'static str {
        match self {
            Kind::Footnote => "footnotePr",
            Kind::Endnote => "endnotePr",
        }
    }
    /// The backing part path, for the finding message.
    fn part(self) -> &'static str {
        match self {
            Kind::Footnote => "word/footnotes.xml",
            Kind::Endnote => "word/endnotes.xml",
        }
    }
}

/// The separator-note ids each notes part contains, or `None` when that part is
/// absent from the package.
pub(super) struct NoteContext {
    pub footnotes: Option<HashSet<String>>,
    pub endnotes: Option<HashSet<String>>,
}

impl NoteContext {
    fn ids_for(&self, kind: Kind) -> Option<&HashSet<String>> {
        match kind {
            Kind::Footnote => self.footnotes.as_ref(),
            Kind::Endnote => self.endnotes.as_ref(),
        }
    }
}

/// Collects the `w:id` values of the `<w:{note}>` elements in a notes part
/// (`footnotes.xml`/`endnotes.xml`), for building a [`NoteContext`]. A part that
/// does not parse yields an empty set (the ordering pass reports malformed XML).
pub(super) fn collect_note_ids(bytes: &[u8], note: &str) -> HashSet<String> {
    let mut ids = HashSet::new();
    if let Ok(nodes) = super::dom::parse(bytes) {
        collect(&nodes, note, &mut ids);
    }
    ids
}

fn collect(nodes: &[Node], note: &str, ids: &mut HashSet<String>) {
    for n in nodes {
        if let Node::Elem(e) = n {
            if e.local() == note {
                if let Some(id) = attr_id(e) {
                    ids.insert(id);
                }
            }
            collect(&e.children, note, ids);
        }
    }
}

/// The `w:id` attribute value of an element, if present.
fn attr_id(e: &Elem) -> Option<String> {
    e.start.attributes().flatten().find_map(|a| {
        (a.key.as_ref() == b"w:id").then(|| String::from_utf8_lossy(&a.value).into_owned())
    })
}

/// Removes dangling footnote/endnote separator references from `settings.xml`.
/// A no-op for every other part.
pub(super) fn fix_note_separators(
    nodes: &mut [Node],
    part: &str,
    ctx: &NoteContext,
    apply: bool,
    findings: &mut Vec<RepairFinding>,
) {
    if part != "word/settings.xml" {
        return;
    }
    walk(nodes, ctx, apply, findings);
}

fn walk(nodes: &mut [Node], ctx: &NoteContext, apply: bool, findings: &mut Vec<RepairFinding>) {
    for n in nodes.iter_mut() {
        if let Node::Elem(e) = n {
            let local = e.local();
            for kind in [Kind::Footnote, Kind::Endnote] {
                if local == kind.pr() {
                    fix_block(e, kind, ctx, apply, findings);
                }
            }
            walk(&mut e.children, ctx, apply, findings);
        }
    }
}

/// Checks one `<w:…Pr>` block and, when `apply`, drops its dangling refs.
fn fix_block(
    e: &mut Elem,
    kind: Kind,
    ctx: &NoteContext,
    apply: bool,
    findings: &mut Vec<RepairFinding>,
) {
    let backing = ctx.ids_for(kind);
    let dangling: Vec<String> = e
        .children
        .iter()
        .filter_map(|c| match c {
            Node::Elem(ce) if ce.local() == kind.note() => attr_id(ce),
            _ => None,
        })
        .filter(|id| !backing.is_some_and(|set| set.contains(id)))
        .collect();
    if dangling.is_empty() {
        return;
    }

    let reason = if backing.is_none() {
        format!("{} is absent", kind.part())
    } else {
        format!("{} does not contain those separator notes", kind.part())
    };
    findings.push(RepairFinding {
        part: "word/settings.xml".to_string(),
        container: format!("w:{}", kind.pr()),
        detail: format!(
            "<w:{}> references separator {}(s) with id(s) {} but {}; Word reports an error \
             in the notes stream — removing the dangling reference(s)",
            kind.pr(),
            kind.note(),
            dangling.join(", "),
            reason,
        ),
    });
    if !apply {
        return;
    }

    let remove: HashSet<String> = dangling.into_iter().collect();
    e.children.retain(|c| match c {
        Node::Elem(ce) if ce.local() == kind.note() => {
            attr_id(ce).is_none_or(|id| !remove.contains(&id))
        }
        _ => true,
    });
}
