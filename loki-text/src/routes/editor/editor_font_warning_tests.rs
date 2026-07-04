// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 AppThere Loki contributors

use std::collections::HashMap;

use super::{build_items, download_link, is_metric_compatible};

#[test]
fn metric_compatible_substitutes_are_low_concern_case_insensitively() {
    assert!(is_metric_compatible("Carlito"));
    assert!(is_metric_compatible("carlito"));
    assert!(is_metric_compatible("Liberation Serif"));
    // A genuine fallback that changes metrics is high-concern.
    assert!(!is_metric_compatible("DejaVu Sans"));
    assert!(!is_metric_compatible("Times New Roman"));
}

#[test]
fn download_link_known_for_office_fonts_only() {
    assert!(download_link("Calibri").is_some());
    assert!(download_link("calibri").is_some()); // case-insensitive
    assert!(download_link("Aptos").is_some());
    assert!(download_link("Comic Sans MS").is_none());
}

#[test]
fn build_items_sorts_and_classifies_severity_and_missing() {
    let mut map: HashMap<String, Option<String>> = HashMap::new();
    map.insert("Calibri".into(), Some("Carlito".into())); // metric-compatible
    map.insert("Aptos".into(), Some("DejaVu Sans".into())); // material fallback
    map.insert("Wingdings".into(), None); // missing, no substitute

    let items = build_items(&map);

    // Sorted by requested name.
    let names: Vec<&str> = items.iter().map(|i| i.requested.as_str()).collect();
    assert_eq!(names, vec!["Aptos", "Calibri", "Wingdings"]);

    // Aptos → DejaVu Sans: a material fallback (not metric-compatible).
    assert!(!items[0].compatible);
    assert_eq!(items[0].substitute.as_deref(), Some("DejaVu Sans"));
    assert!(items[0].link.is_some(), "Aptos has a download link");

    // Calibri → Carlito: metric-compatible (low concern).
    assert!(items[1].compatible);

    // Wingdings: missing, no substitute, no link.
    assert!(items[2].substitute.is_none());
    assert!(!items[2].compatible, "missing is not 'compatible'");
    assert!(items[2].link.is_none());
}
