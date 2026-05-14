// SPDX-License-Identifier: Apache-2.0

//! Compile-time embedding of Fluent translation files.

use rust_embed::RustEmbed;

/// All `.ftl` translation files embedded at compile time.
///
/// Files are stored at `{locale}/{domain}.ftl` relative to the `i18n/`
/// folder (e.g. `en-US/shell.ftl`).
#[derive(RustEmbed)]
#[folder = "i18n/"]
pub struct LokiTranslations;
