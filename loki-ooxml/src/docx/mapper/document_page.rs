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
use loki_doc_model::layout::page::{PageLayout, PageMargins, PageOrientation, PageSize};
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

    layout
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
