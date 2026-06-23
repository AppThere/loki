// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 AppThere Loki contributors

//! Bundled document templates for the Loki suite.
//!
//! Five built-in templates — a Markdown-styled blank, an APA 7 paper, an MLA 9
//! paper, a Hollywood screenplay, and a basic resume — are authored as
//! [`loki_doc_model::document::Document`] builders and shipped as generated
//! `.dotx` assets (regenerate with `cargo run -p loki-templates --bin
//! gen_templates`). [`document`] returns the bundled document for a template id;
//! the editor opens it as a new untitled document.

#![forbid(unsafe_code)]

mod apa;
mod assets;
mod helpers;
mod markdown;
mod mla;
mod resume;
mod screenplay;

use loki_doc_model::document::Document;

/// Stable id + presentation metadata for a bundled template.
pub struct TemplateInfo {
    /// Short stable id, used in untitled paths and `document(id)`.
    pub id: &'static str,
    /// English display name (apps may localise via their own i18n keys).
    pub name: &'static str,
    /// Short format label for the gallery card.
    pub format: &'static str,
}

/// All bundled templates, in gallery order.
pub const TEMPLATES: &[TemplateInfo] = &[
    TemplateInfo {
        id: "markdown",
        name: "Markdown",
        format: "DOTX",
    },
    TemplateInfo {
        id: "apa",
        name: "APA Paper",
        format: "DOTX",
    },
    TemplateInfo {
        id: "mla",
        name: "MLA Paper",
        format: "DOTX",
    },
    TemplateInfo {
        id: "screenplay",
        name: "Screenplay",
        format: "DOTX",
    },
    TemplateInfo {
        id: "resume",
        name: "Resume",
        format: "DOTX",
    },
];

/// Builds template `id`'s document programmatically — the source of truth from
/// which the `.dotx` assets are generated. Returns `None` for an unknown id.
#[must_use]
pub fn build_document(id: &str) -> Option<Document> {
    Some(match id {
        "markdown" => markdown::build(),
        "apa" => apa::build(),
        "mla" => mla::build(),
        "screenplay" => screenplay::build(),
        "resume" => resume::build(),
        _ => return None,
    })
}

/// Returns the bundled document for template `id`, imported from its `.dotx`
/// asset, or `None` for an unknown id.
#[must_use]
pub fn document(id: &str) -> Option<Document> {
    assets::document_from_asset(id)
}
