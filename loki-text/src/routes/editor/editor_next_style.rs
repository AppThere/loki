// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 AppThere Loki contributors

//! Next-paragraph-style resolution for the Enter/split path.
//!
//! A paragraph style's `next_style_id` names the style the paragraph *after*
//! it should take when Enter creates one — `Heading 1` → `Normal`, screenplay
//! `Character` → `Dialogue`. The lookup cannot use
//! [`loki_doc_model::get_block_style_name`]'s return directly: that is a
//! *display* key (`"Heading 1"`, `"Default Paragraph Style"`) while the
//! catalog is keyed by [`StyleId`] (`"Heading1"`…) — the same mismatch
//! `editor_style_target` resolves for the paragraph dialog. This module is the
//! read-only sibling of that resolver: it never seeds, because a style with no
//! catalog definition cannot carry a `next_style_id` either.
//!
//! Word semantics, mirrored here: the next style applies only when the split
//! happens at the **end** of the paragraph (creating a fresh empty paragraph);
//! splitting mid-text leaves both halves in the source style. A `next_style_id`
//! pointing back at the resolved style is a no-op — the tail already inherits
//! the source block's style reference from the split copy.

use loki_doc_model::style::{StyleCatalog, StyleId};

/// What the split's tail block should become, resolved from the source block's
/// style. Headings are a block *type* in the model, so a next style naming a
/// canonical heading id maps to a type change rather than a `style_id` write.
#[derive(Debug, PartialEq, Eq)]
pub(super) enum NextBlock {
    /// Convert the tail to a level-N heading (`set_block_type_heading`).
    Heading(u8),
    /// Convert the tail to a styled paragraph carrying this catalog id
    /// (`set_block_type_para` + `set_block_style`).
    Styled(String),
}

/// Resolves the display key of the block being split to the catalog paragraph
/// style it renders through — read-only, no seeding.
fn resolved_style_id(
    catalog: &StyleCatalog,
    key: &str,
    stored_heading_style: Option<String>,
) -> Option<String> {
    let contains = |id: &str| catalog.paragraph_styles.contains_key(&StyleId::new(id));
    if contains(key) {
        return Some(key.to_string());
    }
    if key == "Default Paragraph Style" {
        let def = catalog.default_paragraph_style.as_ref()?;
        return catalog
            .paragraph_styles
            .contains_key(def)
            .then(|| def.as_str().to_string());
    }
    if let Some(level_str) = key.strip_prefix("Heading ") {
        // The stored heading_style wins — it is what the layout resolver
        // consults first — then the canonical Heading{N} id.
        if let Some(stored) = stored_heading_style
            && contains(&stored)
        {
            return Some(stored);
        }
        let canonical = format!("Heading{level_str}");
        return contains(&canonical).then_some(canonical);
    }
    None
}

/// The next-block decision for a split of a block whose display key is `key`:
/// `None` means "leave the tail as the split made it" (no next style defined,
/// or it resolves to the style the tail already carries).
pub(super) fn next_block_for_split(
    catalog: &StyleCatalog,
    key: &str,
    stored_heading_style: Option<String>,
) -> Option<NextBlock> {
    let current = resolved_style_id(catalog, key, stored_heading_style)?;
    let next = catalog
        .paragraph_styles
        .get(&StyleId::new(&current))?
        .next_style_id
        .as_ref()?
        .as_str()
        .to_string();
    if next == current {
        return None;
    }
    Some(match heading_level_of(&next) {
        Some(level) => NextBlock::Heading(level),
        None => NextBlock::Styled(next),
    })
}

/// Maps a canonical heading style id (`Heading3`, or the display form
/// `Heading 3` some importers store) to its level.
fn heading_level_of(id: &str) -> Option<u8> {
    let rest = id.strip_prefix("Heading")?.trim_start();
    let level: u8 = rest.parse().ok()?;
    (1..=6).contains(&level).then_some(level)
}

#[cfg(test)]
#[path = "editor_next_style_tests.rs"]
mod tests;
