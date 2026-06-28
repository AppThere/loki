// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 AppThere Loki contributors

//! BCP-47 language-tag normalization and fallback resolution.
//!
//! The host platform reports a user locale like `en-US`, `en_US`, or `pt-BR`;
//! the catalog keys dictionaries by shorter tags like `en` or `pt-BR`. These
//! helpers normalize a tag and produce a most-specific-first fallback chain so a
//! locale resolves to the closest available dictionary.

/// Normalizes a language tag for comparison: trims, lowercases, and converts
/// `_` separators to `-`.
///
/// This is a comparison normalization only — it does not validate the tag or
/// apply BCP-47 casing conventions (region subtags are conventionally
/// upper-case), because the catalog is matched case-insensitively.
pub fn normalize(tag: &str) -> String {
    tag.trim().replace('_', "-").to_lowercase()
}

/// Produces the fallback chain for a locale, most specific first.
///
/// Each step drops the last `-`-separated subtag, so `en-US` yields
/// `["en-us", "en"]` and `zh-Hant-TW` yields
/// `["zh-hant-tw", "zh-hant", "zh"]`. An empty input yields an empty chain.
pub fn fallback_chain(locale: &str) -> Vec<String> {
    let normalized = normalize(locale);
    if normalized.is_empty() {
        return Vec::new();
    }
    let parts: Vec<&str> = normalized.split('-').filter(|s| !s.is_empty()).collect();
    let mut chain = Vec::with_capacity(parts.len());
    for end in (1..=parts.len()).rev() {
        chain.push(parts[..end].join("-"));
    }
    chain
}

#[cfg(test)]
#[path = "locale_tests.rs"]
mod tests;
