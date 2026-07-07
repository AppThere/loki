// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 AppThere Loki contributors

//! Tests for [`CharProps`] defaults and style-chain merging.

use super::*;

#[test]
fn default_has_all_none() {
    let cp = CharProps::default();
    assert!(cp.font_name.is_none());
    assert!(cp.bold.is_none());
    assert!(cp.color.is_none());
}

#[test]
fn merge_child_wins_for_some() {
    let parent = CharProps {
        font_name: Some("Times New Roman".into()),
        bold: Some(false),
        font_size: Some(Points::new(12.0)),
        ..Default::default()
    };
    let child = CharProps {
        font_name: Some("Arial".into()),
        bold: Some(true),
        ..Default::default()
    };
    let merged = child.merged_with_parent(&parent);
    assert_eq!(merged.font_name.as_deref(), Some("Arial"));
    assert_eq!(merged.bold, Some(true));
    // font_size inherited from parent
    assert_eq!(merged.font_size, Some(Points::new(12.0)));
}
