// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 AppThere Loki contributors

//! Shared spell-check service for the Loki suite binaries.
//!
//! Wraps `loki-spell` with the app-level concerns the three binaries share: a
//! suite-wide on-disk dictionary cache, OS locale detection for the initial
//! language, and an HTTPS [`ReqwestFetcher`] for downloading additional
//! dictionaries. Each app provides a [`SpellService`] into its Dioxus context at
//! startup; `loki-text` additionally feeds the active checker into the layout
//! engine so misspelled words render a squiggle.
//!
//! The service starts with the **bundled** permissive English dictionary, so
//! spell checking works offline and on first run before any download.

mod fetcher;
mod service;

pub use fetcher::ReqwestFetcher;
pub use service::{SpellService, SpellSnapshot};

use std::path::PathBuf;

/// Suite-wide directory caching downloaded dictionaries (shared by all apps so a
/// language downloaded in one is available in the others).
pub(crate) fn dictionaries_dir() -> Option<PathBuf> {
    data_root().map(|d| d.join("AppThere").join("Loki").join("dictionaries"))
}

// On Android CPU the cfg-gated return is always taken; the desktop fallback is
// unreachable on that target but reachable on desktop/GPU.
#[allow(unreachable_code)]
fn data_root() -> Option<PathBuf> {
    #[cfg(target_os = "android")]
    return crate::recent_documents::android_data_dir();

    dirs::data_dir()
}

/// Detects the host locale as a BCP-47 tag (e.g. `"en-US"`), falling back to
/// `"en"` when the platform reports none.
pub fn detected_locale() -> String {
    sys_locale::get_locale().unwrap_or_else(|| "en".to_string())
}
