// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 AppThere Loki contributors

use super::{fallback_chain, normalize};

#[test]
fn normalize_lowercases_and_unifies_separator() {
    assert_eq!(normalize("en_US"), "en-us");
    assert_eq!(normalize("  PT-br "), "pt-br");
}

#[test]
fn fallback_chain_is_most_specific_first() {
    assert_eq!(fallback_chain("en-US"), ["en-us", "en"]);
    assert_eq!(fallback_chain("en"), ["en"]);
    assert_eq!(
        fallback_chain("zh-Hant-TW"),
        ["zh-hant-tw", "zh-hant", "zh"]
    );
}

#[test]
fn fallback_chain_handles_underscores_and_empties() {
    assert_eq!(fallback_chain("de_DE"), ["de-de", "de"]);
    assert!(fallback_chain("").is_empty());
    assert!(fallback_chain("   ").is_empty());
}
