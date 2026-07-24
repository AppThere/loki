// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 AppThere Loki contributors

//! Unit tests for the picker filter parser (macro spec §5.3, Phase 7B).

use super::FileFilter;

#[test]
fn plain_glob_extension() {
    assert_eq!(FileFilter::parse("*.txt").extensions, vec!["txt"]);
}

#[test]
fn semicolon_separated_globs() {
    assert_eq!(
        FileFilter::parse("*.txt;*.CSV").extensions,
        vec!["txt", "csv"]
    );
}

#[test]
fn description_pattern_pair_dedupes() {
    // GetOpenFilename shape — the pattern appears in the description and again as
    // the pattern; the extension is kept once.
    assert_eq!(
        FileFilter::parse("Text Files (*.txt),*.txt").extensions,
        vec!["txt"]
    );
}

#[test]
fn bare_comma_tokens() {
    assert_eq!(FileFilter::parse("txt, csv").extensions, vec!["txt", "csv"]);
}

#[test]
fn wildcard_only_is_any_file() {
    assert!(FileFilter::parse("*.*").extensions.is_empty());
    assert!(FileFilter::parse("").extensions.is_empty());
}
