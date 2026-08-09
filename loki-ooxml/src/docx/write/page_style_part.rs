// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 AppThere Loki contributors

//! Writer for the advisory page-style part (Spec 08 T6.5, D-02).
//!
//! Adds `/word/lokiPageStyles.xml` plus its **package-level** relationship. See
//! [`crate::docx::page_style_part`] for what the part is and why it is advisory.

use loki_doc_model::document::Document;
use loki_opc::Package;
use loki_opc::part::{PartData, PartName};
use loki_opc::relationships::{Relationship, TargetMode};

use crate::docx::page_style_part::{
    MT_PAGE_STYLES, PAGE_STYLE_PART, PageStyleMap, REL_PAGE_STYLES,
};
use crate::error::OoxmlError;

/// Adds the advisory part to `pkg` when `doc` has page-style names worth
/// carrying. Returns whether a part was added, so the caller knows whether to
/// register the content-type override.
///
/// A document whose sections name no page style gets no part: an empty one is
/// something every future reader has to consider and reject.
pub(crate) fn add_page_style_part(pkg: &mut Package, doc: &Document) -> Result<bool, OoxmlError> {
    let Some(map) = PageStyleMap::from_document(doc) else {
        return Ok(false);
    };
    let name = PartName::new(PAGE_STYLE_PART).map_err(OoxmlError::Opc)?;
    pkg.set_part(
        name.clone(),
        PartData::new(map.to_xml().into_bytes(), MT_PAGE_STYLES),
    );
    pkg.relationships_mut()
        .add(Relationship {
            id: "rIdLokiPageStyles".to_string(),
            rel_type: REL_PAGE_STYLES.to_string(),
            target: PAGE_STYLE_PART.trim_start_matches('/').to_string(),
            target_mode: TargetMode::Internal,
        })
        .map_err(OoxmlError::Opc)?;
    pkg.content_type_map_mut()
        .add_override(&name, MT_PAGE_STYLES);
    Ok(true)
}
