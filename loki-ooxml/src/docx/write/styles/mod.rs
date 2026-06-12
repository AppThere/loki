// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 AppThere Loki contributors

//! `word/styles.xml` serializer.
//!
//! Emits `docDefaults`, a `Normal` paragraph style, `Heading1`–`Heading6`,
//! and all named paragraph/character styles from the document's
//! [`StyleCatalog`].
//!
//! ECMA-376 §17.7 (Document Styles).

mod catalog;
mod defaults;
mod headings;
mod props;

use quick_xml::Writer;

use loki_doc_model::style::catalog::StyleCatalog;

use crate::docx::write::xml::{NS_R, NS_W, write_decl, write_end, write_start};

use catalog::write_catalog_styles;
use defaults::{write_doc_defaults, write_normal_style};
use headings::write_heading_styles;

pub(crate) use props::emit_char_props;

/// Serializes the document's style catalog to `word/styles.xml` bytes.
pub(super) fn write_styles_xml(catalog: &StyleCatalog) -> Vec<u8> {
    let mut out = Vec::new();
    let mut w = Writer::new(&mut out);
    let _ = write_decl(&mut w);

    let _ = write_start(&mut w, "w:styles", &[("xmlns:w", NS_W), ("xmlns:r", NS_R)]);

    write_doc_defaults(&mut w);
    write_normal_style(&mut w, catalog);
    write_heading_styles(&mut w);
    write_catalog_styles(&mut w, catalog);

    let _ = write_end(&mut w, "w:styles");
    drop(w);
    out
}
