// SPDX-License-Identifier: Apache-2.0

//! Maps the block under the cursor to the **catalog style id** the paragraph
//! style dialog edits.
//!
//! [`loki_doc_model::get_block_style_name`] returns a *display* key — `"Default
//! Paragraph Style"` for an unstyled `para` block, `"Heading N"` for a heading —
//! while the dialog (and the layout resolver) address styles by [`StyleId`]
//! (`"DefaultParagraphStyle"`, `"Heading1"`…). Passing the display key straight
//! through left the dialog looking up a style that does not exist, so the
//! Paragraph button appeared dead for the two most common block kinds.
//!
//! Resolution mirrors the layout side (`synthesize_heading_para`,
//! [`StyleCatalog::effective_paragraph_style`]): a heading follows its stored
//! `heading_style` name first, then the canonical `Heading{N}` id; an unstyled
//! paragraph follows the catalog's `default_paragraph_style`. When the style a
//! block resolves through has **no definition yet** — a blank document has no
//! default paragraph style, an imported one may lack a heading definition — the
//! definition is seeded first (from [`ParagraphStyle`]'s built-in constructors,
//! the same table `Document::new_blank` uses), so the dialog always opens on
//! the definition the renderer actually consults.

use std::sync::{Arc, Mutex};

use loki_doc_model::style::{ParagraphStyle, StyleCatalog, StyleId};
use loki_doc_model::{get_block_heading_style, get_block_style_name};
use loro::LoroDoc;

use super::editor_style_catalog::catalog_snapshot;
use crate::editing::state::DocumentState;

/// The style the dialog should open on.
pub(super) struct DialogStyleTarget {
    /// The catalog id to pass as the dialog's `style_id`.
    pub id: String,
    /// `true` when the id was seeded into the catalog just now — the caller
    /// must relayout and refresh undo bookkeeping before mounting the dialog,
    /// or the dialog reads a catalog that does not contain its target yet.
    pub seeded: bool,
}

/// Resolves the catalog style to edit for the block at `block_index`, seeding
/// its definition when the catalog lacks one. `None` when there is no block
/// under the cursor or the seed could not be persisted.
pub(super) fn dialog_style_target(
    doc_state: &Arc<Mutex<DocumentState>>,
    loro: &LoroDoc,
    block_index: usize,
) -> Option<DialogStyleTarget> {
    let key = get_block_style_name(loro, block_index);
    if key.is_empty() {
        return None;
    }
    let catalog = catalog_snapshot(doc_state).unwrap_or_default();
    let (id, seed) = resolve_target(&catalog, &key, get_block_heading_style(loro, block_index))?;
    match seed {
        None => Some(DialogStyleTarget { id, seeded: false }),
        Some(mutate) => {
            let mut next = catalog;
            mutate(&mut next);
            // Without a persisted definition the dialog would silently render
            // nothing (its missing-style guard) — report no target instead of
            // a dead open.
            persist(loro, &next).then_some(DialogStyleTarget { id, seeded: true })
        }
    }
}

/// The pure resolution: the id to edit and, when the catalog lacks its
/// definition, the seed that installs it. Split from the Loro write so the
/// decision table is unit-testable without a CRDT.
#[allow(clippy::type_complexity)]
fn resolve_target(
    catalog: &StyleCatalog,
    key: &str,
    stored_heading_style: Option<String>,
) -> Option<(String, Option<Box<dyn FnOnce(&mut StyleCatalog)>>)> {
    let contains = |id: &str| catalog.paragraph_styles.contains_key(&StyleId::new(id));

    // A styled_para's id (or a catalog style literally named like the key).
    if contains(key) {
        return Some((key.to_string(), None));
    }

    if key == "Default Paragraph Style" {
        if let Some(def) = catalog.default_paragraph_style.as_ref()
            && catalog.paragraph_styles.contains_key(def)
        {
            return Some((def.as_str().to_string(), None));
        }
        let style = ParagraphStyle::builtin_default_paragraph();
        let id = style.id.as_str().to_string();
        return Some((
            id,
            Some(Box::new(move |c: &mut StyleCatalog| {
                c.default_paragraph_style = Some(style.id.clone());
                c.paragraph_styles.insert(style.id.clone(), style);
            })),
        ));
    }

    if let Some(level_str) = key.strip_prefix("Heading ") {
        // The heading's own stored style name wins — it is what the layout
        // resolver consults first. A stored name missing from the catalog gets
        // an empty definition under that id (edits then take effect there);
        // inventing built-in heading properties for a foreign name would claim
        // a look the document never had.
        if let Some(stored) = stored_heading_style {
            if contains(&stored) {
                return Some((stored, None));
            }
            let style = ParagraphStyle {
                is_custom: true,
                ..empty_style(&stored)
            };
            return Some((stored, Some(insert_seed(style))));
        }
        let level: u8 = level_str.parse().ok()?;
        let style = ParagraphStyle::builtin_heading(level);
        let id = style.id.as_str().to_string();
        if contains(&id) {
            return Some((id, None));
        }
        return Some((id, Some(insert_seed(style))));
    }

    // A styled_para whose id has no definition (partial import): seed an empty
    // one so the dialog edits the id the block references.
    let style = ParagraphStyle {
        is_custom: true,
        ..empty_style(key)
    };
    Some((key.to_string(), Some(insert_seed(style))))
}

/// An all-inherited style under `id` — no display name invented, no properties
/// claimed.
fn empty_style(id: &str) -> ParagraphStyle {
    ParagraphStyle {
        id: StyleId::new(id),
        display_name: None,
        parent: None,
        linked_char_style: None,
        next_style_id: None,
        para_props: Default::default(),
        char_props: Default::default(),
        is_default: false,
        is_custom: false,
        extensions: Default::default(),
    }
}

/// A seed closure that inserts `style` into the catalog.
fn insert_seed(style: ParagraphStyle) -> Box<dyn FnOnce(&mut StyleCatalog)> {
    Box::new(move |c: &mut StyleCatalog| {
        c.paragraph_styles.insert(style.id.clone(), style);
    })
}

/// Writes the catalog back through Loro as a discrete, undoable transaction —
/// the same contract as `commit_style_to_loro`.
fn persist(loro: &LoroDoc, catalog: &StyleCatalog) -> bool {
    if let Err(e) = loki_doc_model::loro_bridge::write_document_styles(loro, catalog) {
        tracing::warn!("failed to persist seeded style catalog to Loro: {e}");
        return false;
    }
    loro.commit();
    true
}

#[cfg(test)]
#[path = "editor_style_target_tests.rs"]
mod tests;
