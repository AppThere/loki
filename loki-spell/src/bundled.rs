// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 AppThere Loki contributors

//! The dictionary bundled in the crate binary.
//!
//! A single permissive-licensed dictionary (`en`, SCOWL-derived,
//! `(MIT AND BSD)`) is embedded so spell checking works offline and on first
//! run before any locale-specific dictionary has been downloaded. It is also the
//! ultimate fallback when a user's locale has no installed dictionary.
//!
//! Only a permissive dictionary may be bundled (see [`crate::license`]); the
//! license text travels with it for attribution.

/// The language tag of the bundled dictionary.
pub const BUNDLED_TAG: &str = "en";

/// The bundled dictionary's SPDX license expression.
pub const BUNDLED_LICENSE_SPDX: &str = "(MIT AND BSD)";

const BUNDLED_AFF: &str = include_str!("../assets/dictionaries/en/index.aff");
const BUNDLED_DIC: &str = include_str!("../assets/dictionaries/en/index.dic");

/// The bundled dictionary's license/attribution text (SCOWL readme).
pub const BUNDLED_LICENSE_TEXT: &str = include_str!("../assets/dictionaries/en/license");

/// Returns the bundled dictionary's `(aff, dic)` contents, ready for
/// [`crate::SpellChecker::new`].
pub fn bundled_dictionary() -> (&'static str, &'static str) {
    (BUNDLED_AFF, BUNDLED_DIC)
}
