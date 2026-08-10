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
mod blank;
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
        // Not in `TEMPLATES` (no gallery card, no `.dotx` asset): the blank
        // document, reachable through the editor's blank arm.
        "blank" => blank::build(),
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

/// Merges template `id`'s style catalog into `doc` — **only styles `doc`
/// does not already define**, so an importer's own definitions (e.g. seeded
/// list styles) always win. This is how the Markdown/Fountain importers'
/// bare style references (usage audit §12) get their geometry: the importer
/// emits ids, the caller merges the matching template's catalog. Also adopts
/// the template's page geometry when the document carries none of its own
/// (plain-text sources have no page setup). No-op for an unknown id.
pub fn merge_template_styles(doc: &mut Document, id: &str) {
    let Some(template) = build_document(id) else {
        return;
    };
    let t = template.styles;
    for (k, v) in t.paragraph_styles {
        doc.styles.paragraph_styles.entry(k).or_insert(v);
    }
    for (k, v) in t.character_styles {
        doc.styles.character_styles.entry(k).or_insert(v);
    }
    for (k, v) in t.list_styles {
        doc.styles.list_styles.entry(k).or_insert(v);
    }
    for (k, v) in t.page_styles {
        doc.styles.page_styles.entry(k).or_insert(v);
    }
    for (k, v) in t.table_styles {
        doc.styles.table_styles.entry(k).or_insert(v);
    }
    // Page geometry: a plain-text import carries `Document::new`'s default
    // layout; adopting the template's makes the import lay out like the
    // template (a screenplay's 1.5in text margin, say).
    if let (Some(dst), Some(src)) = (doc.sections.first_mut(), template.sections.first()) {
        dst.layout = src.layout.clone();
        dst.page_style = src.page_style.clone();
    }
}
