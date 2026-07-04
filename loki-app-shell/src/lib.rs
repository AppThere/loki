// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 AppThere Loki contributors

//! Shared application-shell logic for the Loki suite binaries
//! (`loki-text`, `loki-spreadsheet`, `loki-presentation`).
//!
//! Each application is an independent Dioxus binary, but their shell chrome
//! shares the same plumbing: an [`tabs::OpenTab`] model, blank-document creation
//! ([`new_document::new_blank_tab`]) using the `untitled-` path scheme
//! ([`untitled`]), and a persisted recent-documents list
//! ([`recent_documents::RecentDocuments`]).
//!
//! This crate holds the format-neutral, app-agnostic parts of that plumbing so
//! the three binaries do not each carry a copy. Application-specific bits — the
//! document model, routing, ribbon content, and the recent-list file name — stay
//! in each binary.

#![forbid(unsafe_code)]

// The shared Android entry-point macro. `#[macro_export]` hoists `android_main!`
// to the crate root (`loki_app_shell::android_main!`). The macro body's `unsafe`
// tokens are emitted into the *caller*, so this crate stays `forbid(unsafe_code)`.
mod android;

pub mod new_document;
pub mod recent_documents;
pub mod spell;
pub mod tabs;
pub mod untitled;

pub use untitled::{
    NewDocSource, UNTITLED_SCHEME, import_path, is_untitled, parse_new_doc_source, template_path,
};
