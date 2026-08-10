// SPDX-License-Identifier: Apache-2.0

//! The font picker's list model: one list, bundled faces first (design notes
//! 07 and 08).
//!
//! # Why one list and not two pickers
//!
//! Bundled faces sort first under their own heading and carry the proprietary
//! family they replace as a secondary label — the answer to "which one is
//! Arial?" without a second lookup. Device faces stay in the *same* list below
//! them, so nothing is hidden; they are simply the ones whose appearance Loki
//! cannot promise on someone else's reader.

use loki_fonts::{bundled_families, is_bundled_family};

/// One row in the picker.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct FontRow {
    /// The family name, as the layout engine resolves it.
    pub name: String,
    /// For a bundled face, the proprietary family it stands in for. `None` for
    /// a device face.
    pub substitutes_for: Option<String>,
    /// Whether the face ships with Loki — drives the `Bundled` badge and the
    /// grouping heading.
    pub bundled: bool,
}

/// The picker's two sections, in display order.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub(super) struct FontSections {
    /// Faces that ship with Loki, sorted by name.
    pub bundled: Vec<FontRow>,
    /// Faces found on this device, sorted by name, with the bundled ones
    /// removed so no family appears twice.
    pub device: Vec<FontRow>,
}

impl FontSections {
    /// `true` when the filter matched nothing in either section.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.bundled.is_empty() && self.device.is_empty()
    }
}

/// Builds the picker sections from the device's families, filtered by `query`.
///
/// `query` is matched case-insensitively against both the family name and the
/// face it substitutes for, so typing `arial` finds **Arimo** — the substitution
/// is the thing a user is looking for when they type a proprietary name, and a
/// picker that returned nothing for `arial` would send them to a device face
/// that may not exist on their reader's machine.
#[must_use]
pub(super) fn font_sections(device_families: &[String], query: &str) -> FontSections {
    let q = query.trim().to_lowercase();
    let matches = |name: &str, substitutes: Option<&str>| {
        q.is_empty()
            || name.to_lowercase().contains(&q)
            || substitutes.is_some_and(|s| s.to_lowercase().contains(&q))
    };

    let bundled: Vec<FontRow> = bundled_families()
        .iter()
        .filter(|f| matches(f.name, Some(f.substitutes_for)))
        .map(|f| FontRow {
            name: f.name.to_string(),
            substitutes_for: Some(f.substitutes_for.to_string()),
            bundled: true,
        })
        .collect();

    let mut device: Vec<FontRow> = device_families
        .iter()
        // A bundled face installed on the device must not appear twice; the
        // bundled section already offers it, with the better guarantee.
        .filter(|name| !is_bundled_family(name))
        .filter(|name| matches(name, None))
        .map(|name| FontRow {
            name: name.clone(),
            substitutes_for: None,
            bundled: false,
        })
        .collect();
    device.sort_by_key(|r| r.name.to_lowercase());
    device.dedup_by(|a, b| a.name.eq_ignore_ascii_case(&b.name));

    FontSections { bundled, device }
}

#[cfg(test)]
#[path = "font_list_tests.rs"]
mod tests;
