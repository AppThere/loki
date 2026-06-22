// SPDX-License-Identifier: Apache-2.0

//! Blank-document creation for `loki-text` — re-exported from the shared
//! [`loki_app_shell::new_document`] / [`loki_app_shell::untitled`] modules.

pub use loki_app_shell::new_document::{new_blank_tab, new_import_tab, new_template_tab};
pub use loki_app_shell::untitled::{
    NewDocSource, UNTITLED_SCHEME, is_untitled, parse_new_doc_source,
};
