// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 AppThere Loki contributors

//! Page geometry: the named sizes, the default layout, and page usage.

use super::*;

#[test]
fn a4_dimensions() {
    let size = PageSize::a4();
    // A4 is approximately 595 × 842 pt
    assert!((size.width.value() - 595.28).abs() < 0.1);
    assert!((size.height.value() - 841.89).abs() < 0.1);
}

#[test]
fn default_page_layout_portrait() {
    let layout = PageLayout::default();
    assert_eq!(layout.orientation, PageOrientation::Portrait);
    assert!(layout.header.is_none());
}
