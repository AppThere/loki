// Copyright 2026 AppThere Loki contributors
// SPDX-License-Identifier: MIT

//! Relationship part logical addressing calculations configuring relationships layout identifiers.

use crate::part::PartName;

/// Resolves standard relationships metadata layouts.
pub fn relationships_part_for(part: &PartName) -> PartName {
    let name_str = part.as_str();
    let (dir, filename) = match name_str.rsplit_once('/') {
        Some((d, f)) => (d, f),
        None => ("", name_str),
    };
    // Format: directory/_rels/filename.rels
    PartName::new_unchecked(format!("{}/_rels/{}.rels", dir, filename)).unwrap()
}

/// Package top level configuration relationships mapping struct definition.
pub fn package_relationships_part() -> PartName {
    PartName::new_unchecked("/_rels/.rels".to_string()).unwrap()
}
