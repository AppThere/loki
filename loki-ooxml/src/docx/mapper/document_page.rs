// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 AppThere Loki contributors

//! Section / page-layout mapping for the DOCX document mapper (split from
//! `document.rs` for the 300-line ceiling): maps `w:sectPr` geometry
//! (size/margins/orientation, page-number format, section-break type) to a
//! `PageLayout`, and populates the first/even/default header & footer
//! variants from the resolved header/footer parts. `map_section_start` and
//! `map_page_layout_with_hf` are re-imported by `document.rs`.

use std::collections::HashMap;

use loki_doc_model::content::block::Block;
use loki_doc_model::layout::header_footer::{HeaderFooter, HeaderFooterKind};
use loki_doc_model::layout::page::{
    PageBorders, PageLayout, PageMargins, PageOrientation, PageSize,
};
use loki_doc_model::layout::section::SectionStart;
use loki_doc_model::style::list_style::NumberingScheme;
use loki_primitives::units::Points;

use super::{MappingContext, map_paragraph};
use crate::docx::model::paragraph::{DocxParagraph, DocxSectPr};

// ── Page layout ───────────────────────────────────────────────────────────────

/// Maps a `w:sectPr/w:type @w:val` token to a [`SectionStart`].
pub(super) fn map_section_start(section_type: Option<&str>) -> SectionStart {
    match section_type {
        Some("continuous") => SectionStart::Continuous,
        Some("evenPage") => SectionStart::EvenPage,
        Some("oddPage") => SectionStart::OddPage,
        // "nextPage" or absent (the default).
        _ => SectionStart::NewPage,
    }
}

/// Converts a [`DocxSectPr`] to a [`PageLayout`]; A4 portrait, 72pt margins
/// when no `w:sectPr` is present (the OOXML default for simple documents).
fn map_page_layout(sect_pr: Option<&DocxSectPr>) -> PageLayout {
    let Some(sp) = sect_pr else {
        return PageLayout {
            page_size: PageSize::a4(),
            ..Default::default()
        };
    };

    let mut layout = PageLayout::default();

    if let Some(ref pg_sz) = sp.pg_sz {
        let is_landscape = pg_sz.orient.as_deref() == Some("landscape");
        // Some producers store landscape pages with portrait w/h and rely on
        // orient="landscape"; normalise so width is the wider dimension.
        let (w, h) = if is_landscape && pg_sz.w < pg_sz.h {
            (pg_sz.h, pg_sz.w)
        } else {
            (pg_sz.w, pg_sz.h)
        };
        layout.page_size = PageSize {
            width: Points::new(f64::from(w) / 20.0),
            height: Points::new(f64::from(h) / 20.0),
        };
        layout.orientation = if is_landscape {
            PageOrientation::Landscape
        } else {
            PageOrientation::Portrait
        };
    }

    if let Some(ref pg_mar) = sp.pg_mar {
        layout.margins = PageMargins {
            top: Points::new(f64::from(pg_mar.top) / 20.0),
            bottom: Points::new(f64::from(pg_mar.bottom) / 20.0),
            left: Points::new(f64::from(pg_mar.left) / 20.0),
            right: Points::new(f64::from(pg_mar.right) / 20.0),
            header: Points::new(f64::from(pg_mar.header) / 20.0),
            footer: Points::new(f64::from(pg_mar.footer) / 20.0),
            gutter: Points::new(f64::from(pg_mar.gutter) / 20.0),
        };
    }

    layout.columns = crate::docx::mapper::document_cols::map_columns(sp.cols.as_ref());

    // Page numbering: format (roman/alpha) and restart value from w:pgNumType.
    layout.page_number_format = sp.pg_num_fmt.as_deref().map(map_page_num_fmt);
    layout.page_number_start = sp.pg_num_start;

    layout.page_border = sp
        .pg_borders
        .as_ref()
        .map(map_pg_borders)
        .filter(|b| !b.is_empty());

    layout
}

/// Maps a parsed `w:pgBorders` set to the doc-model [`PageBorders`], dropping
/// edges that are absent or explicitly `none`/`nil`.
fn map_pg_borders(b: &crate::docx::model::section::DocxPgBorders) -> PageBorders {
    use loki_doc_model::style::props::border::BorderStyle;
    let edge = |e: &Option<crate::docx::model::paragraph::DocxBorderEdge>| {
        e.as_ref()
            .map(crate::docx::mapper::props::map_border_edge)
            .filter(|bd| bd.style != BorderStyle::None)
    };
    PageBorders {
        top: edge(&b.top),
        left: edge(&b.left),
        bottom: edge(&b.bottom),
        right: edge(&b.right),
        offset_from_text: b.offset_from_text,
    }
}

/// Maps an OOXML `w:pgNumType @w:fmt` token to a [`NumberingScheme`].
///
/// Unknown formats fall back to decimal (ECMA-376 §17.6.12 lists the same
/// `w:numFmt` token set used for list numbering).
fn map_page_num_fmt(fmt: &str) -> NumberingScheme {
    match fmt {
        "lowerRoman" => NumberingScheme::LowerRoman,
        "upperRoman" => NumberingScheme::UpperRoman,
        "lowerLetter" => NumberingScheme::LowerAlpha,
        "upperLetter" => NumberingScheme::UpperAlpha,
        _ => NumberingScheme::Decimal,
    }
}

// ── Header / footer helpers ───────────────────────────────────────────────────

fn map_hf_blocks(
    paragraphs: &[DocxParagraph],
    kind: HeaderFooterKind,
    ctx: &mut MappingContext<'_>,
) -> HeaderFooter {
    let blocks: Vec<Block> = paragraphs
        .iter()
        .flat_map(|p| map_paragraph(p, ctx))
        .collect();
    HeaderFooter { kind, blocks }
}

/// Converts a [`DocxSectPr`] to a [`PageLayout`], populating header/footer
/// variants from `header_parts`/`footer_parts` (keyed by relationship ID).
///
/// `even_and_odd` mirrors `w:evenAndOddHeaders` in `w:settings`.
pub(super) fn map_page_layout_with_hf(
    sect_pr: Option<&DocxSectPr>,
    header_parts: &HashMap<String, Vec<DocxParagraph>>,
    footer_parts: &HashMap<String, Vec<DocxParagraph>>,
    even_and_odd: bool,
    ctx: &mut MappingContext<'_>,
) -> PageLayout {
    let mut layout = map_page_layout(sect_pr);

    let Some(sp) = sect_pr else {
        return layout;
    };

    for hf_ref in &sp.header_refs {
        if let Some(paras) = header_parts.get(&hf_ref.rel_id) {
            match hf_ref.hf_type.as_str() {
                "default" => {
                    layout.header = Some(map_hf_blocks(paras, HeaderFooterKind::Default, ctx));
                }
                "first" if sp.title_page => {
                    layout.header_first = Some(map_hf_blocks(paras, HeaderFooterKind::First, ctx));
                }
                "even" if even_and_odd => {
                    layout.header_even = Some(map_hf_blocks(paras, HeaderFooterKind::Even, ctx));
                }
                _ => {}
            }
        }
    }

    for hf_ref in &sp.footer_refs {
        if let Some(paras) = footer_parts.get(&hf_ref.rel_id) {
            match hf_ref.hf_type.as_str() {
                "default" => {
                    layout.footer = Some(map_hf_blocks(paras, HeaderFooterKind::Default, ctx));
                }
                "first" if sp.title_page => {
                    layout.footer_first = Some(map_hf_blocks(paras, HeaderFooterKind::First, ctx));
                }
                "even" if even_and_odd => {
                    layout.footer_even = Some(map_hf_blocks(paras, HeaderFooterKind::Even, ctx));
                }
                _ => {}
            }
        }
    }

    layout
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::docx::model::paragraph::DocxBorderEdge;
    use crate::docx::model::section::DocxPgBorders;

    fn edge() -> DocxBorderEdge {
        DocxBorderEdge {
            val: "single".into(),
            sz: Some(8),
            color: Some("4472C4".into()),
            space: Some(24),
        }
    }

    #[test]
    fn maps_page_borders_onto_the_layout() {
        let sect = DocxSectPr {
            pg_borders: Some(DocxPgBorders {
                top: Some(edge()),
                bottom: Some(edge()),
                left: Some(edge()),
                right: Some(edge()),
                offset_from_text: false,
            }),
            ..Default::default()
        };
        let layout = map_page_layout(Some(&sect));
        let pb = layout.page_border.expect("page border mapped");
        assert!(pb.top.is_some() && pb.left.is_some() && pb.bottom.is_some() && pb.right.is_some());
        assert!(!pb.offset_from_text);
    }

    #[test]
    fn all_none_edges_map_to_no_border() {
        let sect = DocxSectPr {
            pg_borders: Some(DocxPgBorders::default()),
            ..Default::default()
        };
        assert!(map_page_layout(Some(&sect)).page_border.is_none());
    }
}
