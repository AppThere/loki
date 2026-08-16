// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 AppThere Loki contributors

//! ODT export writers: a [`loki_doc_model::document::Document`] is serialised to
//! the `content.xml`, `styles.xml`, and `meta.xml` parts of an ODT package,
//! along with any embedded image parts.

mod auto;
mod content;
mod default_style;
mod inlines;
mod list_write;
mod media;
mod meta;
mod page_styles;
mod para_props;
mod props;
mod revisions;
mod styles;
mod table_style;
mod tables;
mod xml;

pub(crate) use content::content_xml;
pub(crate) use media::{MathPart, MediaPart};
pub(crate) use meta::meta_xml;
pub(crate) use styles::styles_xml;

/// Whether `id` is a synthetic internal style rather than a real ODF style
/// name.
///
/// The model carries the document's family defaults as `__`-prefixed entries
/// (`__Default`, `__DefaultChar`, `__DefaultTable`, and OOXML's
/// `__DocDefault*`). They are serialised as `<style:default-style>`, never as a
/// named `<style:style>`, so neither their *definitions* nor any *reference* to
/// them may reach the output — a `style:parent-style-name="__Default"` names a
/// style the package does not contain.
///
/// One predicate rather than a `starts_with("__")` at each site: the definition
/// side already had three copies, and the reference side had none, which is how
/// re-export came to emit a dangling parent.
pub(super) fn is_synthetic_style_id(id: &str) -> bool {
    id.starts_with("__")
}
