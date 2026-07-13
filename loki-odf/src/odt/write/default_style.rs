// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 AppThere Loki contributors

//! `<style:default-style>` writer (ODF 1.3 §16.4): the document-wide
//! paragraph and text (character) defaults, emitted as the first children of
//! `<office:styles>`.
//!
//! These are the ODF form of the imported defaults the catalog carries as the
//! `is_default` paragraph style (ODT `__Default` / DOCX `__DocDefault`) and
//! [`StyleCatalog::default_character_style`] (`__DefaultChar` /
//! `__DocDefaultChar`) — synthetic `__`-prefixed entries that are deliberately
//! *skipped* by the named-style writer. Before this writer existed they were
//! silently dropped on export, so a re-opened document lost its base font,
//! size, and paragraph defaults (5.10 tail).

use loki_doc_model::style::catalog::StyleCatalog;

use super::para_props::emit_paragraph_properties;
use super::props::emit_text_properties;

/// Writes the paragraph- and text-family `<style:default-style>` elements for
/// `catalog` into `out`. A family with no imported default is omitted.
pub(super) fn write_default_styles(out: &mut String, catalog: &StyleCatalog) {
    // Paragraph family: the catalog's single `is_default` style (at most one,
    // per the `ParagraphStyle::is_default` invariant).
    if let Some(style) = catalog.paragraph_styles.values().find(|s| s.is_default) {
        out.push_str("<style:default-style style:family=\"paragraph\">");
        out.push_str(&emit_paragraph_properties(&style.para_props));
        out.push_str(&emit_text_properties(&style.char_props));
        out.push_str("</style:default-style>");
    }
    // Text family: the run-default character style (ADR-0012 Decision 1).
    if let Some(cs) = catalog
        .default_character_style
        .as_ref()
        .and_then(|id| catalog.character_styles.get(id))
    {
        out.push_str("<style:default-style style:family=\"text\">");
        out.push_str(&emit_text_properties(&cs.char_props));
        out.push_str("</style:default-style>");
    }
}
