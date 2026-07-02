// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 AppThere Loki contributors

//! Paragraph-style hierarchy queries for the inheritance tree view and its
//! impact preview (Spec 05 M4).
//!
//! The single-parent tree of [`resolve`](crate::style::resolve) has an
//! *upward* view (`para_ancestors`, `para_reparent_cycles`); this module adds the
//! *downward* view — direct children, transitive descendants, and the
//! **exact dependent set affected** by changing one property on a base style.
//! All walks are cycle/depth-guarded so a corrupt catalog can never loop.

use std::collections::HashSet;

use crate::style::catalog::{MAX_STYLE_CHAIN_DEPTH, StyleCatalog, StyleId};
use crate::style::para_style::ParagraphStyle;

impl StyleCatalog {
    /// The direct children of paragraph style `id` (styles whose `parent` is
    /// `id`), in catalog order.
    #[must_use]
    pub fn para_children(&self, id: &StyleId) -> Vec<StyleId> {
        self.paragraph_styles
            .iter()
            .filter(|(_, s)| s.parent.as_ref() == Some(id))
            .map(|(cid, _)| cid.clone())
            .collect()
    }

    /// All transitive descendants of paragraph style `id` (breadth-first, nearest
    /// generation first), excluding `id` itself. Cycle- and depth-guarded.
    #[must_use]
    pub fn para_descendants(&self, id: &StyleId) -> Vec<StyleId> {
        let mut out = Vec::new();
        let mut seen = HashSet::new();
        seen.insert(id.clone());
        let mut frontier = self.para_children(id);
        // Bound the number of generations walked; a catalog cannot have a longer
        // legitimate chain than `MAX_STYLE_CHAIN_DEPTH`.
        for _ in 0..=MAX_STYLE_CHAIN_DEPTH {
            if frontier.is_empty() {
                break;
            }
            let mut next = Vec::new();
            for child in frontier {
                if seen.insert(child.clone()) {
                    next.extend(self.para_children(&child));
                    out.push(child);
                }
            }
            frontier = next;
        }
        out
    }

    /// The **exact set of descendants whose value of a property would change** if
    /// that property were changed on `base` — the impact preview (Spec 05 §7).
    ///
    /// `has_local` reports whether a style sets the property locally (e.g.
    /// `|s| s.para_props.alignment.is_some()`). A descendant `D` is affected iff
    /// no style *strictly between* `D` and `base` (inclusive of `D`, exclusive of
    /// `base`) overrides the property — i.e. `D` currently inherits the property
    /// from `base`, or would newly pick it up from `base`. Descendants shadowed
    /// by a closer override are excluded. Returned in `para_descendants` order.
    #[must_use]
    pub fn dependents_affected(
        &self,
        base: &StyleId,
        has_local: impl Fn(&ParagraphStyle) -> bool,
    ) -> Vec<StyleId> {
        self.para_descendants(base)
            .into_iter()
            .filter(|d| self.inherits_property_from(d, base, &has_local))
            .collect()
    }

    /// Whether `descendant` reaches `base` without an intervening local override
    /// of the property tested by `has_local` (walking nearest-first from
    /// `descendant`). Cycle/depth safety comes from [`Self::para_ancestors`].
    fn inherits_property_from(
        &self,
        descendant: &StyleId,
        base: &StyleId,
        has_local: &impl Fn(&ParagraphStyle) -> bool,
    ) -> bool {
        for ancestor in self.para_ancestors(descendant) {
            if &ancestor == base {
                return true; // reached base with no closer override
            }
            if self.paragraph_styles.get(&ancestor).is_some_and(has_local) {
                return false; // a closer style overrides the property
            }
        }
        false // base is not on the chain (shouldn't happen for a descendant)
    }
}

#[cfg(test)]
#[path = "tree_tests.rs"]
mod tests;
