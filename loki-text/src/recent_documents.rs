// SPDX-License-Identifier: Apache-2.0

//! Recent-documents persistence for `loki-text` — re-exported from the shared
//! [`loki_app_shell::recent_documents`] module, with this app's storage file
//! name. Pass [`RECENT_FILE`] to `RecentDocuments::load`.

pub use loki_app_shell::recent_documents::*;

/// Relative path under the platform data directory for this app's recent list.
pub const RECENT_FILE: &str = "AppThere/Loki/recent.json";
