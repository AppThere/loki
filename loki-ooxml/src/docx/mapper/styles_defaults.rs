// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 AppThere Loki contributors

//! Where `w:docDefaults` sit in the style hierarchy.

use loki_doc_model::style::StyleId;
use loki_doc_model::style::catalog::StyleCatalog;

// Every style without a `w:basedOn` still sits *on* `w:docDefaults`: they
// are the lowest level of the style hierarchy (ECMA-376 §17.7.2), not a
// fallback for documents that omit `Normal`. Parenting only the
// *synthesised* `Normal` to `__DocDefault` left every real, root-level style
// resolving against engine defaults instead of the document's.
//
// Invisible in `iris-blueprint.docx` twice over — its runs carry explicit
// `w:rFonts`, and its `docDefaults` font is Arial, which is also Loki's own
// fallback. `acid2-docx.docx` declares `Calibri`/22 half-points and a bare
// `Normal`, and rendered as 12 pt Arial: 9 % wider than Word, so every body
// line broke early.
pub(super) fn root_parent_resolver(
    catalog: &StyleCatalog,
) -> impl Fn(Option<&str>, &StyleId) -> Option<StyleId> + use<> {
    let doc_default = catalog
        .paragraph_styles
        .contains_key(&StyleId::new("__DocDefault"))
        .then(|| StyleId::new("__DocDefault"));
    move |based_on: Option<&str>, own: &StyleId| match based_on {
        Some(b) => Some(StyleId::new(b)),
        // `__DocDefault` must not parent itself. Defensive only: the catalog
        // truncates a cycle at `MAX_STYLE_CHAIN_DEPTH` and `__DocDefault` has
        // nothing above it to inherit, so removing this guard changes no
        // resolved value — it keeps the graph acyclic rather than fixing an
        // observable defect.
        None if Some(own) != doc_default.as_ref() => doc_default.clone(),
        None => None,
    }
}
