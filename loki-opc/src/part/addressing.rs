// Copyright 2024-2026 AppThere
//
// Licensed under the MIT License.

//! Resolving pack IRI values inside package configurations.

use crate::part::PartName;

/// Normalizes relative IRI segments tracking relational states to resolve fully qualified `PartName` limits.
pub fn resolve_relative_reference(base: &PartName, target: &str) -> Option<PartName> {
    if target.starts_with('/') {
        return PartName::new(target).ok();
    }

    let base_str = base.as_str();
    let dir = match base_str.rsplit_once('/') {
        Some((d, _)) => d,
        None => "",
    };

    let mut path = dir.to_string();
    let segments = target.split('/');

    for segment in segments {
        if segment == "." || segment.is_empty() {
            continue;
        } else if segment == ".." {
            if let Some((d, _)) = path.rsplit_once('/') {
                path = d.to_string();
            } else {
                path.clear();
            }
        } else {
            path.push('/');
            path.push_str(segment);
        }
    }

    if !path.starts_with('/') {
        path.insert(0, '/');
    }

    PartName::new(path).ok()
}
