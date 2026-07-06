// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 AppThere Loki contributors

//! Provenance-aware resolution for the **table** style family (Spec 05 M6,
//! ADR-0012 Decision 1).
//!
//! The table analogue of the character resolver in [`resolve`](crate::style::resolve):
//! same single-parent chain, same `Local / Inherited / Default / FormatDefault`
//! provenance, with the `Default` level supplied by the document
//! [`default_table_style`](StyleCatalog::default_table_style). Split into its own
//! module so `resolve.rs` stays under the 300-line ceiling; the shared
//! [`Resolved`] constructors are `pub(crate)` for exactly this.

use std::collections::HashSet;

use crate::style::catalog::{MAX_STYLE_CHAIN_DEPTH, StyleCatalog, StyleId};
use crate::style::resolve::Resolved;
use crate::style::table_style::TableStyle;

impl StyleCatalog {
    /// Resolves one property over a **table style's** inheritance chain, with
    /// provenance — the table-family analogue of
    /// [`resolve_char_chain`](StyleCatalog::resolve_char_chain). Levels: `Local`,
    /// then the explicit `parent` chain (`Inherited`), then the document
    /// [`default_table_style`](StyleCatalog::default_table_style) (`Default`),
    /// else `FormatDefault`. Cycle- and depth-guarded.
    ///
    /// Returns `None` only when `id` is not a table style in the catalog.
    pub fn resolve_table_chain<T: Clone>(
        &self,
        id: &StyleId,
        get: impl Fn(&TableStyle) -> Option<T>,
    ) -> Option<Resolved<T>> {
        let style = self.table_styles.get(id)?;
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
            let Some(parent) = self.table_styles.get(pid) else {
                break;
            };
            if let Some(v) = get(parent) {
                return Some(Resolved::inherited(pid.clone(), v));
            }
            cursor = parent.parent.as_ref();
        }
        if let Some(def) = self.default_table_style.as_ref()
            && !visited.contains(def)
            && let Some(v) = self.first_in_table_chain(def, &get)
        {
            return Some(Resolved::from_default(v));
        }
        Some(Resolved::format_default())
    }

    /// First local value of a property along a table chain starting at `start`
    /// (inclusive), cycle/depth-guarded. Backs the table family's `Default` lookup.
    fn first_in_table_chain<T: Clone>(
        &self,
        start: &StyleId,
        get: &impl Fn(&TableStyle) -> Option<T>,
    ) -> Option<T> {
        let mut visited = HashSet::new();
        let mut cursor = Some(start.clone());
        for _ in 0..MAX_STYLE_CHAIN_DEPTH {
            let id = cursor?;
            if !visited.insert(id.clone()) {
                return None;
            }
            let style = self.table_styles.get(&id)?;
            if let Some(v) = get(style) {
                return Some(v);
            }
            cursor = style.parent.clone();
        }
        None
    }

    /// The table style's ancestors, nearest-first and **including** `id` itself,
    /// stopping at the root or the first repeat (cycle/depth-guarded).
    #[must_use]
    pub fn table_ancestors(&self, id: &StyleId) -> Vec<StyleId> {
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
                .table_styles
                .get(&current)
                .and_then(|s| s.parent.clone());
        }
        chain
    }

    /// Whether making `new_parent` the parent of `child` would create a cycle in
    /// the table-style tree (the analogue of
    /// [`para_reparent_cycles`](StyleCatalog::para_reparent_cycles)).
    #[must_use]
    pub fn table_reparent_cycles(&self, child: &StyleId, new_parent: &StyleId) -> bool {
        self.table_ancestors(new_parent).iter().any(|a| a == child)
    }
}
