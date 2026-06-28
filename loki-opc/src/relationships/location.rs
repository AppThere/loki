// SPDX-License-Identifier: MIT
// Copyright 2026 AppThere Loki contributors

//! Computes the conventional `.rels` part name for a given part (§6.5.1).

use crate::part::PartName;

/// Returns the relationships part name for `part`
/// (`<dir>/_rels/<file>.rels`).
pub fn relationships_part_for(part: &PartName) -> PartName {
    let name_str = part.as_str();
    let (dir, filename) = match name_str.rsplit_once('/') {
        Some((d, f)) => (d, f),
        None => ("", name_str),
    };
    // Format: directory/_rels/filename.rels
    PartName::new_unchecked(format!("{}/_rels/{}.rels", dir, filename)).unwrap()
}

/// Returns the package-level relationships part name (`/_rels/.rels`).
pub fn package_relationships_part() -> PartName {
    PartName::new_unchecked("/_rels/.rels".to_string()).unwrap()
}
