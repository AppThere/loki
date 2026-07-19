// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 AppThere Loki contributors

//! Styles mapper: [`DocxStyles`] → [`StyleCatalog`].

use loki_doc_model::content::attr::ExtensionBag;
use loki_doc_model::style::catalog::{StyleCatalog, StyleId};
use loki_doc_model::style::char_style::CharacterStyle;
use loki_doc_model::style::para_style::ParagraphStyle;
use loki_doc_model::style::props::char_props::CharProps;
use loki_doc_model::style::props::para_props::ParaProps;
use loki_doc_model::style::table_borders::TableBorders;
use loki_doc_model::style::table_style::{
    TableConditionalFormat, TableProps, TableRegion, TableStyle,
};
use loki_primitives::color::DocumentColor;

use indexmap::IndexMap;

use crate::docx::model::styles::{DocxStyleType, DocxStyles, DocxTableStyleProps};

use super::props::{map_ppr, map_rpr};

/// Translates a [`DocxStyles`] collection into a [`StyleCatalog`].
///
/// Document defaults (`w:docDefaults`) are synthesised as a special
/// `ParagraphStyle` with id `"__DocDefault"` and `is_default = true`;
/// it serves as the root of the inheritance chain.
///
/// Table styles carry band sizes, base cell shading, and `w:tblStylePr`
/// conditional (banding/region) shading; numbering styles are skipped silently.
pub(crate) fn map_styles(styles: &DocxStyles) -> StyleCatalog {
    let mut catalog = StyleCatalog::new();

    // Synthesise a root default style from w:docDefaults.
    if styles.default_ppr.is_some() || styles.default_rpr.is_some() {
        let default_style = ParagraphStyle {
            id: StyleId::new("__DocDefault"),
            display_name: None,
            parent: None,
            linked_char_style: None,
            next_style_id: None,
            para_props: styles.default_ppr.as_ref().map(map_ppr).unwrap_or_default(),
            char_props: styles.default_rpr.as_ref().map(map_rpr).unwrap_or_default(),
            is_default: true,
            is_custom: false,
            extensions: ExtensionBag::default(),
        };
        catalog
            .paragraph_styles
            .insert(StyleId::new("__DocDefault"), default_style);
    }

    // Synthesise the document's **default character style** from the same
    // `w:rPrDefault` run defaults (ADR-0012 Decision 1 — the character family's
    // `Default` source), so a standalone character style resolves docDefaults
    // properties as `Provenance::Default` instead of `FormatDefault`. Distinct
    // from the paragraph `__DocDefault` because the character resolver walks the
    // `character_styles` catalog, not the paragraph one.
    if let Some(rpr) = styles.default_rpr.as_ref() {
        let id = StyleId::new("__DocDefaultChar");
        catalog.character_styles.insert(
            id.clone(),
            CharacterStyle {
                id: id.clone(),
                display_name: None,
                parent: None,
                char_props: map_rpr(rpr),
                extensions: ExtensionBag::default(),
            },
        );
        catalog.default_character_style = Some(id);
    }

    for style in &styles.styles {
        let id = StyleId::new(&style.style_id);
        match style.style_type {
            DocxStyleType::Paragraph => {
                let s = ParagraphStyle {
                    id: id.clone(),
                    display_name: style.name.clone(),
                    parent: style.based_on.as_deref().map(StyleId::new),
                    linked_char_style: style.link.as_deref().map(StyleId::new),
                    next_style_id: style.next.clone(),
                    para_props: style.ppr.as_ref().map(map_ppr).unwrap_or_default(),
                    char_props: style.rpr.as_ref().map(map_rpr).unwrap_or_default(),
                    is_default: style.is_default,
                    is_custom: style.is_custom,
                    extensions: ExtensionBag::default(),
                };
                // COMPAT(microsoft): duplicate styleId — last definition wins,
                // matching Microsoft Word's behavior per §2.7.3.17.
                catalog.paragraph_styles.insert(id, s);
            }
            DocxStyleType::Character => {
                let s = CharacterStyle {
                    id: id.clone(),
                    display_name: style.name.clone(),
                    parent: style.based_on.as_deref().map(StyleId::new),
                    char_props: style.rpr.as_ref().map(map_rpr).unwrap_or_default(),
                    extensions: ExtensionBag::default(),
                };
                // COMPAT(microsoft): duplicate styleId — last definition wins,
                // matching Microsoft Word's behavior per §2.7.3.17.
                catalog.character_styles.insert(id, s);
            }
            DocxStyleType::Table => {
                // Band sizes + base cell shading + `w:tblStylePr` conditional
                // (banding/region) shading; `w:tblLook` selects the active
                // regions per table instance (not yet imported).
                let (table_props, conditional) = style
                    .table
                    .as_ref()
                    .map(map_table_style_props)
                    .unwrap_or_default();
                let s = TableStyle {
                    id: id.clone(),
                    display_name: style.name.clone(),
                    parent: style.based_on.as_deref().map(StyleId::new),
                    table_props,
                    conditional,
                    extensions: ExtensionBag::default(),
                };
                // The table style flagged `w:default="1"` (e.g. TableNormal) is
                // the table family's `Default` source (ADR-0012 Decision 1).
                if style.is_default {
                    catalog.default_table_style = Some(id.clone());
                }
                // COMPAT(microsoft): duplicate styleId — last definition wins,
                // matching Microsoft Word's behavior per §2.7.3.17.
                catalog.table_styles.insert(id, s);
            }
            DocxStyleType::Numbering => {
                // Numbering styles are expressed through w:abstractNum/w:num;
                // these are handled by map_numbering in the numbering module.
            }
        }
    }

    // COMPAT(microsoft): Normal style missing from styles.xml —
    // synthesize from docDefaults per OOXML §2.7.3.
    // This is common in programmatically generated documents.
    // Note: If the styles part is completely missing (empty), we should not synthesize it.
    let has_any_defined_style =
        !styles.styles.is_empty() || styles.default_ppr.is_some() || styles.default_rpr.is_some();
    if has_any_defined_style
        && !catalog
            .paragraph_styles
            .contains_key(&StyleId::new("Normal"))
    {
        let parent = if catalog
            .paragraph_styles
            .contains_key(&StyleId::new("__DocDefault"))
        {
            Some(StyleId::new("__DocDefault"))
        } else {
            None
        };
        let normal_style = ParagraphStyle {
            id: StyleId::new("Normal"),
            display_name: Some("Normal".into()),
            parent,
            linked_char_style: None,
            next_style_id: None,
            para_props: ParaProps::default(),
            char_props: CharProps::default(),
            is_default: true,
            is_custom: false,
            extensions: ExtensionBag::default(),
        };
        catalog
            .paragraph_styles
            .insert(StyleId::new("Normal"), normal_style);
    }

    // Record the document's default paragraph style — the one a bare paragraph
    // (no `w:pStyle`) inherits from. Prefer the explicit `w:default="1"`
    // paragraph style; fall back to the canonical/synthesized `Normal`. Through
    // its parent chain this reaches `__DocDefault` (w:docDefaults), so default-
    // font body text picks up the document base font instead of engine defaults.
    catalog.default_paragraph_style = styles
        .styles
        .iter()
        .find(|s| matches!(s.style_type, DocxStyleType::Paragraph) && s.is_default)
        .map(|s| StyleId::new(&s.style_id))
        .or_else(|| {
            catalog
                .paragraph_styles
                .contains_key(&StyleId::new("Normal"))
                .then(|| StyleId::new("Normal"))
        });

    catalog
}

/// Map an OOXML `w:tblStylePr @w:type` region string to a [`TableRegion`].
fn map_table_region(region: &str) -> Option<TableRegion> {
    Some(match region {
        "wholeTable" => TableRegion::WholeTable,
        "firstRow" => TableRegion::FirstRow,
        "lastRow" => TableRegion::LastRow,
        "firstCol" => TableRegion::FirstColumn,
        "lastCol" => TableRegion::LastColumn,
        "band1Horz" => TableRegion::Band1Horz,
        "band2Horz" => TableRegion::Band2Horz,
        "band1Vert" => TableRegion::Band1Vert,
        "band2Vert" => TableRegion::Band2Vert,
        "nwCell" => TableRegion::NwCell,
        "neCell" => TableRegion::NeCell,
        "swCell" => TableRegion::SwCell,
        "seCell" => TableRegion::SeCell,
        _ => return None,
    })
}

/// A shading fill hex (no `#`) → `DocumentColor`. `auto`/absent → `None`.
fn shd_color(fill: Option<&str>) -> Option<DocumentColor> {
    crate::xml_util::resolve_shading(fill, None, None).map(DocumentColor::Rgb)
}

/// Build [`TableProps`] and the conditional-region map from parsed table-style
/// data. Regions carrying neither shading nor character formatting are
/// skipped.
fn map_table_style_props(
    t: &DocxTableStyleProps,
) -> (TableProps, IndexMap<TableRegion, TableConditionalFormat>) {
    let borders = t
        .tbl_borders
        .as_ref()
        .map(map_tbl_borders)
        .filter(|b| !b.is_empty());
    let table_props = TableProps {
        background_color: shd_color(t.base_shd_fill.as_deref()),
        row_band_size: t.row_band_size,
        col_band_size: t.col_band_size,
        borders,
        ..TableProps::default()
    };
    let mut conditional = IndexMap::new();
    for c in &t.conditional {
        let Some(region) = map_table_region(&c.region) else {
            continue;
        };
        let background_color = shd_color(c.shd_fill.as_deref());
        let char_props = c
            .rpr
            .as_ref()
            .map(super::props::map_rpr)
            .unwrap_or_default();
        if background_color.is_none() && char_props == CharProps::default() {
            continue;
        }
        conditional.insert(
            region,
            TableConditionalFormat {
                background_color,
                char_props,
            },
        );
    }
    (table_props, conditional)
}

/// Maps a parsed `w:tblBorders` set to the doc-model [`TableBorders`], dropping
/// edges that are absent or explicitly `none`/`nil` (they draw nothing).
fn map_tbl_borders(b: &crate::docx::model::styles::DocxTblBorders) -> TableBorders {
    use crate::docx::model::paragraph::DocxBorderEdge;
    use loki_doc_model::style::props::border::BorderStyle;
    let edge = |e: &Option<DocxBorderEdge>| {
        e.as_ref()
            .map(super::props::map_border_edge)
            .filter(|bd| bd.style != BorderStyle::None)
    };
    TableBorders {
        top: edge(&b.top),
        left: edge(&b.left),
        bottom: edge(&b.bottom),
        right: edge(&b.right),
        inside_h: edge(&b.inside_h),
        inside_v: edge(&b.inside_v),
    }
}

// ── Tests ─────────────────────────────────────────────────────────────────────

#[cfg(test)]
#[path = "styles_tests.rs"]
mod tests;
