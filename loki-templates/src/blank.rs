// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 AppThere Loki contributors

//! The blank document, as a template.
//!
//! Markdown's style catalog with an empty body — so a blank document opens
//! with a real body font, heading styles, and next-style chains instead of
//! the bare heading-only catalog `Document::new_blank` used to provide. Not
//! in [`crate::TEMPLATES`]: the gallery's Blank card and the shell's `+`
//! reach it through the editor's blank arm, which also applies the user's
//! page-geometry defaults (a seeding only *new* documents get).

use loki_doc_model::document::Document;

use crate::helpers::{assemble, letter_layout, p};
use crate::markdown;

/// Builds the blank template: Markdown's catalog, one empty body paragraph.
pub(crate) fn build() -> Document {
    let body = vec![p("Normal", "")];
    let mut doc = assemble("", letter_layout(1.0), markdown::styles(), body);
    // A blank document has no title until the author gives it one — an empty
    // string would still export as a <dc:title>.
    doc.meta.title = None;
    doc
}
