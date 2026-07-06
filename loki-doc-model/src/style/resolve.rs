// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 AppThere Loki contributors

//! Provenance-aware style resolution (Spec 05 M1).
//!
//! The collapsing resolvers in [`catalog`](crate::style::catalog)
//! (`resolve_para` / `resolve_char`) answer "what is the effective value?" for
//! the renderer. The style-management panel (Spec 05 §6) needs more: for each
//! property, *where the value comes from* — set locally, inherited from a named
//! ancestor, the document default, or an engine fallback. This module adds that
//! provenance over the same single-parent tree, plus the cycle guard
//! re-parenting (§7) needs.
//!
//! Resolution is generic over a *getter* that reads one property's local value
//! from a style, so a single method serves every property of a family without a
//! giant per-property result struct: the inspector iterates its property list
//! and resolves each row on demand.

use std::collections::HashSet;

use crate::style::catalog::{MAX_STYLE_CHAIN_DEPTH, StyleCatalog, StyleId};
use crate::style::char_style::CharacterStyle;
use crate::style::para_style::ParagraphStyle;

/// Where a resolved property value comes from, relative to the queried style.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Provenance {
    /// Set directly on the queried style.
    Local,
    /// Set by the named ancestor in the style's own `parent` chain.
    Inherited(StyleId),
    /// Not set anywhere in the style's chain; supplied by the document default
    /// style (the OOXML `docDefaults` / ODF default-style fall-through).
    Default,
    /// Unset everywhere — the engine/format fallback decides the value.
    FormatDefault,
}

/// A resolved property: where it came from and its value.
///
/// `value` is `None` only for [`Provenance::FormatDefault`], where the model
/// holds no value and the rendering engine supplies the fallback.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Resolved<T> {
    /// Where the value originates.
    pub provenance: Provenance,
    /// The resolved value (`None` ⇔ `FormatDefault`).
    pub value: Option<T>,
}

impl<T> Resolved<T> {
    fn local(value: T) -> Self {
        Self {
            provenance: Provenance::Local,
            value: Some(value),
        }
    }

    fn inherited(from: StyleId, value: T) -> Self {
        Self {
            provenance: Provenance::Inherited(from),
            value: Some(value),
        }
    }

    fn from_default(value: T) -> Self {
        Self {
            provenance: Provenance::Default,
            value: Some(value),
        }
    }

    fn format_default() -> Self {
        Self {
            provenance: Provenance::FormatDefault,
            value: None,
        }
    }

    /// `true` when the value is set on the queried style itself (an override the
    /// inspector lets the user reset to inherited).
    #[must_use]
    pub fn is_local(&self) -> bool {
        matches!(self.provenance, Provenance::Local)
    }
}

impl StyleCatalog {
    /// Resolves one property over a **paragraph style's** inheritance chain, with
    /// provenance. `get` reads the property's *local* value from a style
    /// (e.g. `|s| s.para_props.alignment.clone()` or `|s| s.char_props.bold`),
    /// so this one method serves both paragraph properties and the run-default
    /// character properties of a paragraph style.
    ///
    /// Order: the style itself (`Local`), then its `parent` chain (`Inherited`),
    /// then the document default paragraph style if it is not already in that
    /// chain (`Default`), else `FormatDefault`. Cycle- and depth-guarded.
    ///
    /// Returns `None` only when `id` is not a paragraph style in the catalog.
    pub fn resolve_para_chain<T: Clone>(
        &self,
        id: &StyleId,
        get: impl Fn(&ParagraphStyle) -> Option<T>,
    ) -> Option<Resolved<T>> {
        let style = self.paragraph_styles.get(id)?;
        if let Some(v) = get(style) {
            return Some(Resolved::local(v));
        }

        // Walk the explicit `parent` chain → Inherited.
        let mut visited = HashSet::new();
        visited.insert(id.clone());
        let mut cursor = style.parent.as_ref();
        for _ in 0..MAX_STYLE_CHAIN_DEPTH {
            let Some(pid) = cursor else { break };
            if !visited.insert(pid.clone()) {
                break; // cycle
            }
            let Some(parent) = self.paragraph_styles.get(pid) else {
                break;
            };
            if let Some(v) = get(parent) {
                return Some(Resolved::inherited(pid.clone(), v));
            }
            cursor = parent.parent.as_ref();
        }

        // Fall through to the document default style (when not already visited).
        if let Some(def) = self.default_paragraph_style.as_ref()
            && !visited.contains(def)
            && let Some(v) = self.first_in_para_chain(def, &get)
        {
            return Some(Resolved::from_default(v));
        }

        Some(Resolved::format_default())
    }

    /// First local value of a property along a paragraph chain starting at
    /// `start` (inclusive), cycle/depth-guarded. Backs the `Default`-level lookup.
    fn first_in_para_chain<T: Clone>(
        &self,
        start: &StyleId,
        get: &impl Fn(&ParagraphStyle) -> Option<T>,
    ) -> Option<T> {
        let mut visited = HashSet::new();
        let mut cursor = Some(start.clone());
        for _ in 0..MAX_STYLE_CHAIN_DEPTH {
            let id = cursor?;
            if !visited.insert(id.clone()) {
                return None;
            }
            let style = self.paragraph_styles.get(&id)?;
            if let Some(v) = get(style) {
                return Some(v);
            }
            cursor = style.parent.clone();
        }
        None
    }

    /// Resolves one property over a **character style's** inheritance chain, with
    /// provenance — the standalone-`CharacterStyle` resolver the collapsing
    /// `resolve_char` never provided (that one walks paragraph styles). The
    /// character family has no document default, so the levels are `Local`,
    /// `Inherited`, then `FormatDefault`. Cycle- and depth-guarded.
    ///
    /// Returns `None` only when `id` is not a character style in the catalog.
    pub fn resolve_char_chain<T: Clone>(
        &self,
        id: &StyleId,
        get: impl Fn(&CharacterStyle) -> Option<T>,
    ) -> Option<Resolved<T>> {
        let style = self.character_styles.get(id)?;
        if let Some(v) = get(style) {
            return Some(Resolved::local(v));
        }
        let mut visited = HashSet::new();
        visited.insert(id.clone());
        let mut cursor = style.parent.as_ref();
        for _ in 0..MAX_STYLE_CHAIN_DEPTH {
            let Some(pid) = cursor else { break };
            if !visited.insert(pid.clone()) {
                break; // cycle
            }
            let Some(parent) = self.character_styles.get(pid) else {
                break;
            };
            if let Some(v) = get(parent) {
                return Some(Resolved::inherited(pid.clone(), v));
            }
            cursor = parent.parent.as_ref();
        }

        // Fall through to the document default character style (ADR-0012 Decision
        // 1 — the per-family `Default` source), when set and not already visited.
        if let Some(def) = self.default_character_style.as_ref()
            && !visited.contains(def)
            && let Some(v) = self.first_in_char_chain(def, &get)
        {
            return Some(Resolved::from_default(v));
        }

        Some(Resolved::format_default())
    }

    /// First local value of a property along a character chain starting at
    /// `start` (inclusive), cycle/depth-guarded. Backs the character family's
    /// `Default`-level lookup (mirrors [`first_in_para_chain`](Self::first_in_para_chain)).
    fn first_in_char_chain<T: Clone>(
        &self,
        start: &StyleId,
        get: &impl Fn(&CharacterStyle) -> Option<T>,
    ) -> Option<T> {
        let mut visited = HashSet::new();
        let mut cursor = Some(start.clone());
        for _ in 0..MAX_STYLE_CHAIN_DEPTH {
            let id = cursor?;
            if !visited.insert(id.clone()) {
                return None;
            }
            let style = self.character_styles.get(&id)?;
            if let Some(v) = get(style) {
                return Some(v);
            }
            cursor = style.parent.clone();
        }
        None
    }

    /// The paragraph style's ancestors, nearest-first and **including** `id`
    /// itself, stopping at the root or the first repeat (cycle/depth-guarded).
    #[must_use]
    pub fn para_ancestors(&self, id: &StyleId) -> Vec<StyleId> {
        let mut chain = Vec::new();
        let mut seen = HashSet::new();
        let mut cursor = Some(id.clone());
        for _ in 0..=MAX_STYLE_CHAIN_DEPTH {
            let Some(current) = cursor else { break };
            if !seen.insert(current.clone()) {
                break; // cycle
            }
            chain.push(current.clone());
            cursor = self
                .paragraph_styles
                .get(&current)
                .and_then(|s| s.parent.clone());
        }
        chain
    }

    /// Whether making `new_parent` the parent of `child` would create a cycle in
    /// the paragraph-style tree — i.e. `new_parent` is `child` itself or a
    /// descendant of `child`. Re-parenting (Spec 05 §7) must reject these to keep
    /// the hierarchy a tree.
    #[must_use]
    pub fn para_reparent_cycles(&self, child: &StyleId, new_parent: &StyleId) -> bool {
        // A cycle forms iff `child` lies on `new_parent`'s ancestor chain
        // (which includes `new_parent` itself, covering `new_parent == child`).
        self.para_ancestors(new_parent).iter().any(|a| a == child)
    }

    /// The character style's ancestors, nearest-first and **including** `id`
    /// itself, stopping at the root or the first repeat (cycle/depth-guarded).
    /// The character-family analogue of [`para_ancestors`](Self::para_ancestors).
    #[must_use]
    pub fn char_ancestors(&self, id: &StyleId) -> Vec<StyleId> {
        let mut chain = Vec::new();
        let mut seen = HashSet::new();
        let mut cursor = Some(id.clone());
        for _ in 0..=MAX_STYLE_CHAIN_DEPTH {
            let Some(current) = cursor else { break };
            if !seen.insert(current.clone()) {
                break; // cycle
            }
            chain.push(current.clone());
            cursor = self
                .character_styles
                .get(&current)
                .and_then(|s| s.parent.clone());
        }
        chain
    }

    /// Whether making `new_parent` the parent of `child` would create a cycle in
    /// the character-style tree (the analogue of
    /// [`para_reparent_cycles`](Self::para_reparent_cycles)). The character-style
    /// editor must reject these to keep the family a tree (Spec 05 §7).
    #[must_use]
    pub fn char_reparent_cycles(&self, child: &StyleId, new_parent: &StyleId) -> bool {
        self.char_ancestors(new_parent).iter().any(|a| a == child)
    }
}

#[cfg(test)]
#[path = "resolve_tests.rs"]
mod tests;
