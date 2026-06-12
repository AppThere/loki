// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 AppThere Loki contributors

//! Mapping context threaded through all document-mapping helpers.

use std::collections::HashMap;

use loki_doc_model::content::block::Block;
use loki_doc_model::style::catalog::StyleCatalog;
use loki_primitives::units::Points;

use crate::error::OdfWarning;
use crate::odt::import::OdtImportOptions;
use crate::odt::model::styles::OdfCellProps;

/// State threaded through all mapping helpers during a single
/// [`super::map_document`] call.
///
/// Holds read-only references to the resolved catalog, image store, and import
/// options, plus mutable collections for warnings and for floating figures that
/// were encountered inside inline content and need to be emitted as block-level
/// siblings after their host paragraph.
pub(crate) struct OdfMappingContext<'a> {
    /// The fully-built style catalog (paragraph, character, list styles).
    pub styles: &'a StyleCatalog,
    /// Images extracted from the ODF package: ZIP-entry path →
    /// (media-type, raw bytes).
    pub images: &'a HashMap<String, (String, Vec<u8>)>,
    /// Import options controlling heading emission, image embedding, etc.
    pub options: &'a OdtImportOptions,
    /// Column widths from `style:table-column-properties`: style name → points.
    /// Pre-built from the ODF stylesheet before the mapping pass.
    pub col_style_widths: &'a HashMap<String, Points>,
    /// Cell properties from `style:table-cell-properties`: style name → props.
    /// Pre-built from the ODF stylesheet before the mapping pass.
    pub cell_style_props: &'a HashMap<String, OdfCellProps>,
    /// Non-fatal issues accumulated during mapping.
    pub warnings: Vec<OdfWarning>,
    /// Floating frames (images and text boxes that are not `as-char` anchored)
    /// collected while mapping inline content. The caller flushes this after
    /// each paragraph or block.
    pub pending_figures: Vec<Block>,
}
