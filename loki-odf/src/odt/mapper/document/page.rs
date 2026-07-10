// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 AppThere Loki contributors

//! Page-layout mapping: master-page resolution, header/footer assignment, and
//! `style:page-layout` → [`PageLayout`] conversion.

use std::collections::HashMap;

use loki_doc_model::layout::header_footer::{HeaderFooter, HeaderFooterKind};
use loki_doc_model::layout::page::{
    PageLayout, PageMargins, PageOrientation, PageSize, SectionColumns,
};
use loki_doc_model::style::list_style::NumberingScheme;
use loki_primitives::units::Points;

use crate::odt::model::document::{OdfColumns, OdfMasterPage, OdfPageLayout};
use crate::odt::model::paragraph::OdfParagraph;
use crate::odt::model::styles::{OdfStyle, OdfStylesheet};
use crate::xml_util::parse_length;

use super::OdfMappingContext;
use super::inlines::map_paragraph;

/// Resolves the effective master page name for a paragraph style, following
/// the `style:parent-style-name` inheritance chain.
///
/// Returns `None` when no master page transition is defined anywhere in the
/// chain. A cycle in the parent chain terminates the walk without a result.
pub(super) fn resolve_master_page_name<'a>(
    style_name: &str,
    all_styles: &'a HashMap<&str, &'a OdfStyle>,
) -> Option<String> {
    let mut current = style_name;
    let mut depth = 0usize;
    loop {
        // Guard against malformed cycles in the style inheritance chain.
        if depth > 32 {
            break;
        }
        depth += 1;
        let style = all_styles.get(current)?;
        if let Some(ref mpn) = style.master_page_name
            && !mpn.is_empty()
        {
            return Some(mpn.clone());
        }
        current = style.parent_name.as_deref()?;
    }
    None
}

/// Build a [`PageLayout`] for the named master page.
///
/// Looks up the named master page in `stylesheet.master_pages`. If
/// `master_name` is `None`, falls back to the "Standard" / "Default" master,
/// then the first one. Converts the associated `style:page-layout` to a
/// format-neutral [`PageLayout`] and populates all header/footer variants.
/// Returns [`PageLayout::default`] when no master page is found.
pub(super) fn resolve_page_layout_by_name(
    stylesheet: &OdfStylesheet,
    master_name: Option<&str>,
    ctx: &mut OdfMappingContext<'_>,
) -> PageLayout {
    let master = master_name
        .and_then(|name| stylesheet.master_pages.iter().find(|m| m.name == name))
        .or_else(|| {
            stylesheet
                .master_pages
                .iter()
                .find(|m| m.name == "Standard" || m.name == "Default")
        })
        .or_else(|| stylesheet.master_pages.first());

    let odf_layout = master.and_then(|m| {
        stylesheet
            .page_layouts
            .iter()
            .find(|pl| pl.name == m.page_layout_name)
    });

    let mut layout = match odf_layout {
        Some(pl) => convert_page_layout(pl),
        None => PageLayout::default(),
    };

    if let Some(master) = master {
        apply_master_page_hf(master, &mut layout, ctx);
    }

    layout
}

/// Map all header/footer variants from `master` onto `layout`.
fn apply_master_page_hf(
    master: &OdfMasterPage,
    layout: &mut PageLayout,
    ctx: &mut OdfMappingContext<'_>,
) {
    layout.header = map_hf_paras(master.header.as_ref(), HeaderFooterKind::Default, ctx);
    layout.footer = map_hf_paras(master.footer.as_ref(), HeaderFooterKind::Default, ctx);
    layout.header_first = map_hf_paras(master.header_first.as_ref(), HeaderFooterKind::First, ctx);
    layout.footer_first = map_hf_paras(master.footer_first.as_ref(), HeaderFooterKind::First, ctx);
    layout.header_even = map_hf_paras(master.header_even.as_ref(), HeaderFooterKind::Even, ctx);
    layout.footer_even = map_hf_paras(master.footer_even.as_ref(), HeaderFooterKind::Even, ctx);
}

/// Convert a list of [`OdfParagraph`]s into a [`HeaderFooter`].
///
/// Returns `None` when `paras` is `None` or empty (preserving the "absent
/// variant" semantics that [`assign_headers_footers`] relies on).
///
/// [`assign_headers_footers`]: loki_layout::flow::assign_headers_footers
fn map_hf_paras(
    paras: Option<&Vec<OdfParagraph>>,
    kind: HeaderFooterKind,
    ctx: &mut OdfMappingContext<'_>,
) -> Option<HeaderFooter> {
    let paras = paras?;
    if paras.is_empty() {
        return None;
    }
    let blocks = paras.iter().map(|p| map_paragraph(p, ctx)).collect();
    Some(HeaderFooter { kind, blocks })
}

fn convert_page_layout(pl: &OdfPageLayout) -> PageLayout {
    let zero = Points::new(0.0);
    let width = pl
        .page_width
        .as_deref()
        .and_then(parse_length)
        .unwrap_or_else(|| Points::new(595.28));
    let height = pl
        .page_height
        .as_deref()
        .and_then(parse_length)
        .unwrap_or_else(|| Points::new(841.89));
    let mt = pl
        .margin_top
        .as_deref()
        .and_then(parse_length)
        .unwrap_or_else(|| Points::new(72.0));
    let mb = pl
        .margin_bottom
        .as_deref()
        .and_then(parse_length)
        .unwrap_or_else(|| Points::new(72.0));
    let ml = pl
        .margin_left
        .as_deref()
        .and_then(parse_length)
        .unwrap_or_else(|| Points::new(72.0));
    let mr = pl
        .margin_right
        .as_deref()
        .and_then(parse_length)
        .unwrap_or_else(|| Points::new(72.0));

    let orientation = match pl.print_orientation.as_deref() {
        Some("landscape") => PageOrientation::Landscape,
        _ => PageOrientation::Portrait,
    };

    // Multi-column layout is only meaningful for two or more columns.
    let content_width = Points::new((width.value() - ml.value() - mr.value()).max(0.0));
    let columns = pl
        .columns
        .as_ref()
        .filter(|c| c.count >= 2)
        .map(|c| map_columns(c, content_width));

    // `style:num-format` on `style:page-layout-properties` selects the page-number
    // numbering scheme (decimal / roman / alpha); the PAGE field is formatted
    // through the shared list-marker converter at substitution time. Decimal is
    // the renderer default, so it is left unset rather than carried explicitly.
    let page_number_format = pl
        .num_format
        .as_deref()
        .map(crate::odt::mapper::lists::map_numbering_scheme)
        .filter(|s| *s != NumberingScheme::Decimal);

    PageLayout {
        page_size: PageSize { width, height },
        margins: PageMargins {
            top: mt,
            bottom: mb,
            left: ml,
            right: mr,
            header: Points::new(36.0),
            footer: Points::new(36.0),
            gutter: zero,
        },
        orientation,
        columns,
        page_number_format,
        ..Default::default()
    }
}

/// Map an ODF `style:columns` definition to [`SectionColumns`]. Unequal columns
/// (`style:column @style:rel-width`) distribute `content_width` (minus the
/// inter-column gaps) proportionally to their relative shares; equal columns
/// leave `widths` empty for the layout engine to split evenly.
fn map_columns(c: &OdfColumns, content_width: Points) -> SectionColumns {
    let count = u8::try_from(c.count.clamp(2, u32::from(u8::MAX))).unwrap_or(2);
    let gap = c
        .gap
        .as_deref()
        .and_then(parse_length)
        .unwrap_or_else(|| Points::new(18.0));
    let widths = column_widths(&c.rel_widths, count, gap, content_width);
    SectionColumns {
        count,
        gap,
        separator: c.separator,
        widths,
    }
}

/// Distribute `content_width` across `rel_widths` (relative shares), returning
/// one [`Points`] width per column. Empty unless a share is present for every
/// column and their sum is positive.
fn column_widths(rel_widths: &[f32], count: u8, gap: Points, content_width: Points) -> Vec<Points> {
    if rel_widths.len() != usize::from(count) {
        return Vec::new();
    }
    let total: f64 = rel_widths.iter().map(|s| f64::from(*s)).sum();
    if total <= 0.0 {
        return Vec::new();
    }
    let inner = (content_width.value() - (f64::from(count) - 1.0) * gap.value()).max(0.0);
    rel_widths
        .iter()
        .map(|s| Points::new(inner * f64::from(*s) / total))
        .collect()
}
