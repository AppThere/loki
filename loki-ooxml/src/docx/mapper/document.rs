// Copyright 2026 AppThere Loki contributors
// SPDX-License-Identifier: Apache-2.0

//! Top-level document mapper: orchestrates all sub-mappers and produces a
//! [`loki_doc_model::Document`].

use std::collections::HashMap;

use loki_doc_model::content::attr::ExtensionBag;
use loki_doc_model::content::block::Block;
use loki_doc_model::document::Document;
use loki_doc_model::layout::header_footer::{HeaderFooter, HeaderFooterKind};
use loki_doc_model::layout::page::{PageLayout, PageMargins, PageOrientation, PageSize};
use loki_doc_model::layout::section::Section;
use loki_doc_model::meta::core::DocumentMeta;
use loki_doc_model::style::catalog::StyleCatalog;
use loki_opc::PartData;
use loki_primitives::units::Points;

use crate::docx::import::DocxImportOptions;
use crate::docx::model::document::{DocxBodyChild, DocxDocument};
use crate::docx::model::footnotes::{DocxNoteType, DocxNotes};
use crate::docx::model::numbering::DocxNumbering;
use crate::docx::model::paragraph::{DocxParagraph, DocxSectPr};
use crate::docx::model::settings::DocxSettings;
use crate::docx::model::styles::DocxStyles;
use crate::error::OoxmlWarning;

use super::numbering::map_numbering;
use super::paragraph::map_paragraph;
use super::styles::map_styles;
use super::table::map_table;

// ── Context ───────────────────────────────────────────────────────────────────

/// Shared state threaded through all content-level mappers.
pub(crate) struct MappingContext<'a> {
    /// The resolved style catalog for this document.
    pub styles: &'a StyleCatalog,
    /// Footnote content keyed by `w:footnote @w:id`.
    pub footnotes: &'a HashMap<i32, Vec<Block>>,
    /// Endnote content keyed by `w:endnote @w:id`.
    pub endnotes: &'a HashMap<i32, Vec<Block>>,
    /// External hyperlink targets: relationship id → URL.
    pub hyperlinks: &'a HashMap<String, String>,
    /// Image parts: relationship id → raw bytes + media type.
    pub images: &'a HashMap<String, PartData>,
    /// Import options controlling heading promotion, image embedding, etc.
    pub options: &'a DocxImportOptions,
    /// Non-fatal warnings accumulated during mapping.
    pub warnings: Vec<OoxmlWarning>,
}

// ── Page layout ───────────────────────────────────────────────────────────────

/// Converts a [`DocxSectPr`] to a [`PageLayout`].
///
/// Falls back to A4 portrait with 72pt margins when no `w:sectPr` is
/// present (the OOXML default assumption for simple documents).
fn map_page_layout(sect_pr: Option<&DocxSectPr>) -> PageLayout {
    let Some(sp) = sect_pr else {
        return PageLayout {
            page_size: PageSize::a4(),
            ..Default::default()
        };
    };

    let mut layout = PageLayout::default();

    if let Some(ref pg_sz) = sp.pg_sz {
        layout.page_size = PageSize {
            width: Points::new(f64::from(pg_sz.w) / 20.0),
            height: Points::new(f64::from(pg_sz.h) / 20.0),
        };
        layout.orientation = if pg_sz.orient.as_deref() == Some("landscape") {
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

    layout
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
fn map_page_layout_with_hf(
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
                    layout.header =
                        Some(map_hf_blocks(paras, HeaderFooterKind::Default, ctx));
                }
                "first" if sp.title_page => {
                    layout.header_first =
                        Some(map_hf_blocks(paras, HeaderFooterKind::First, ctx));
                }
                "even" if even_and_odd => {
                    layout.header_even =
                        Some(map_hf_blocks(paras, HeaderFooterKind::Even, ctx));
                }
                _ => {}
            }
        }
    }

    for hf_ref in &sp.footer_refs {
        if let Some(paras) = footer_parts.get(&hf_ref.rel_id) {
            match hf_ref.hf_type.as_str() {
                "default" => {
                    layout.footer =
                        Some(map_hf_blocks(paras, HeaderFooterKind::Default, ctx));
                }
                "first" if sp.title_page => {
                    layout.footer_first =
                        Some(map_hf_blocks(paras, HeaderFooterKind::First, ctx));
                }
                "even" if even_and_odd => {
                    layout.footer_even =
                        Some(map_hf_blocks(paras, HeaderFooterKind::Even, ctx));
                }
                _ => {}
            }
        }
    }

    layout
}

// ── Note pre-processing ───────────────────────────────────────────────────────

/// Maps a notes part to a `HashMap<id, Vec<Block>>` using the given context.
///
/// Only `Normal`-type notes are included; separators and continuation
/// separators are skipped. The context should use empty note maps to avoid
/// circular dependencies (notes referencing notes is not supported in v0.1.0).
fn map_notes_to_blocks(
    notes: Option<&DocxNotes>,
    ctx: &mut MappingContext<'_>,
) -> HashMap<i32, Vec<Block>> {
    let Some(notes) = notes else {
        return HashMap::new();
    };
    notes
        .notes
        .iter()
        .filter(|n| n.note_type == DocxNoteType::Normal)
        .map(|n| {
            let blocks: Vec<Block> = n.paragraphs.iter()
                .flat_map(|p| map_paragraph(p, ctx))
                .collect();
            (n.id, blocks)
        })
        .collect()
}

// ── Metadata ──────────────────────────────────────────────────────────────────

/// Populates [`DocumentMeta`] from OPC core properties.
fn map_meta(core_props: Option<&loki_opc::CoreProperties>) -> DocumentMeta {
    let Some(cp) = core_props else {
        return DocumentMeta::default();
    };
    DocumentMeta {
        title: cp.title.clone(),
        creator: cp.creator.clone(),
        subject: cp.subject.clone(),
        keywords: cp.keywords.clone(),
        description: cp.description.clone(),
        last_modified_by: cp.last_modified_by.clone(),
        created: cp.created,
        modified: cp.modified,
        ..Default::default()
    }
}

// ── Main entry point ──────────────────────────────────────────────────────────

/// Orchestrates the full intermediate-model → [`Document`] translation.
///
/// Steps:
/// 1. Map styles (always present).
/// 2. Map numbering into the catalog (optional part).
/// 3. Pre-process footnote/endnote content maps.
/// 4. Walk body children, splitting on embedded `w:sectPr` elements.
/// 5. Close the final section using the body-level `w:sectPr`.
/// 6. Map core properties into document metadata.
#[allow(clippy::too_many_arguments)]
pub(crate) fn map_document(
    doc: &DocxDocument,
    raw_styles: &DocxStyles,
    raw_numbering: Option<&DocxNumbering>,
    raw_footnotes: Option<&DocxNotes>,
    raw_endnotes: Option<&DocxNotes>,
    images: &HashMap<String, PartData>,
    hyperlinks: &HashMap<String, String>,
    header_parts: &HashMap<String, Vec<DocxParagraph>>,
    footer_parts: &HashMap<String, Vec<DocxParagraph>>,
    raw_settings: Option<&DocxSettings>,
    core_props: Option<&loki_opc::CoreProperties>,
    options: &DocxImportOptions,
) -> (Document, Vec<OoxmlWarning>) {
    // ── 1. Styles ──────────────────────────────────────────────────────────
    let mut catalog = map_styles(raw_styles);
    let mut all_warnings: Vec<OoxmlWarning> = Vec::new();

    // ── 2. Numbering ───────────────────────────────────────────────────────
    if let Some(numbering) = raw_numbering {
        let w = map_numbering(numbering, &mut catalog);
        all_warnings.extend(w);
    }

    // ── 3. Notes ───────────────────────────────────────────────────────────
    // Map note paragraphs using a temporary context with empty note maps.
    // Notes that cross-reference other notes are not supported in v0.1.0.
    let empty_notes: HashMap<i32, Vec<Block>> = HashMap::new();
    let footnote_map = {
        let mut note_ctx = MappingContext {
            styles: &catalog,
            footnotes: &empty_notes,
            endnotes: &empty_notes,
            hyperlinks,
            images,
            options,
            warnings: Vec::new(),
        };
        let result = map_notes_to_blocks(raw_footnotes, &mut note_ctx);
        all_warnings.extend(note_ctx.warnings);
        result
    };
    let endnote_map = {
        let mut note_ctx = MappingContext {
            styles: &catalog,
            footnotes: &empty_notes,
            endnotes: &empty_notes,
            hyperlinks,
            images,
            options,
            warnings: Vec::new(),
        };
        let result = map_notes_to_blocks(raw_endnotes, &mut note_ctx);
        all_warnings.extend(note_ctx.warnings);
        result
    };

    let even_and_odd = raw_settings.is_some_and(|s| s.even_and_odd_headers);

    // ── 4+5. Section-split body walk ───────────────────────────────────────
    let mut ctx = MappingContext {
        styles: &catalog,
        footnotes: &footnote_map,
        endnotes: &endnote_map,
        hyperlinks,
        images,
        options,
        warnings: Vec::new(),
    };

    let mut sections: Vec<Section> = Vec::new();
    let mut current_blocks: Vec<Block> = Vec::new();

    for child in &doc.body.children {
        match child {
            DocxBodyChild::Paragraph(p) => {
                let sect_pr = p.ppr.as_ref().and_then(|ppr| ppr.sect_pr.as_ref());
                let blocks = map_paragraph(p, &mut ctx);
                current_blocks.extend(blocks);
                if let Some(sp) = sect_pr {
                    let layout = map_page_layout_with_hf(
                        Some(sp),
                        header_parts,
                        footer_parts,
                        even_and_odd,
                        &mut ctx,
                    );
                    sections.push(Section {
                        layout,
                        blocks: std::mem::take(&mut current_blocks),
                        extensions: ExtensionBag::default(),
                    });
                }
            }
            DocxBodyChild::Table(t) => {
                let block = map_table(t, &mut ctx);
                current_blocks.push(block);
            }
            DocxBodyChild::Sdt => {}
        }
    }

    // Close the final section.
    let final_layout = map_page_layout_with_hf(
        doc.body.final_sect_pr.as_ref(),
        header_parts,
        footer_parts,
        even_and_odd,
        &mut ctx,
    );
    sections.push(Section {
        layout: final_layout,
        blocks: current_blocks,
        extensions: ExtensionBag::default(),
    });

    all_warnings.extend(ctx.warnings);

    // ── 6. Metadata ────────────────────────────────────────────────────────
    let meta = map_meta(core_props);

    let document = Document {
        meta,
        styles: catalog,
        sections,
        source: None,
    };

    (document, all_warnings)
}

// ── Tests ─────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use crate::docx::model::document::{DocxBody, DocxDocument};
    use crate::docx::model::paragraph::{DocxPPr, DocxParagraph, DocxPgMar, DocxPgSz, DocxSectPr};
    use crate::docx::model::styles::DocxStyles;
    use loki_doc_model::layout::page::PageSize;

    fn empty_doc() -> DocxDocument {
        DocxDocument {
            body: DocxBody { children: vec![], final_sect_pr: None },
        }
    }

    fn sect_pr_a4() -> DocxSectPr {
        DocxSectPr {
            pg_sz: Some(DocxPgSz { w: 11906, h: 16838, orient: None }),
            pg_mar: Some(DocxPgMar {
                top: 1440,
                bottom: 1440,
                left: 1440,
                right: 1440,
                header: 720,
                footer: 720,
                gutter: 0,
            }),
            header_refs: vec![],
            footer_refs: vec![],
            title_page: false,
        }
    }

    fn run_map(doc: &DocxDocument, final_sect: Option<DocxSectPr>) -> (Document, Vec<OoxmlWarning>) {
        let mut d = doc.clone();
        d.body.final_sect_pr = final_sect;
        map_document(
            &d,
            &DocxStyles::default(),
            None,
            None,
            None,
            &HashMap::new(),
            &HashMap::new(),
            &HashMap::new(),
            &HashMap::new(),
            None,
            None,
            &DocxImportOptions::default(),
        )
    }

    #[test]
    fn single_section_produced_for_no_section_breaks() {
        let (doc, _) = run_map(&empty_doc(), Some(sect_pr_a4()));
        assert_eq!(doc.sections.len(), 1);
    }

    #[test]
    fn two_sections_when_mid_document_sect_pr() {
        let sect_pr = sect_pr_a4();
        let para_with_break = DocxBodyChild::Paragraph(DocxParagraph {
            ppr: Some(DocxPPr {
                sect_pr: Some(sect_pr_a4()),
                ..Default::default()
            }),
            children: vec![],
        });
        let doc = DocxDocument {
            body: DocxBody {
                children: vec![para_with_break],
                final_sect_pr: None,
            },
        };
        let (mapped, _) = map_document(
            &doc,
            &DocxStyles::default(),
            None,
            None,
            None,
            &HashMap::new(),
            &HashMap::new(),
            &HashMap::new(),
            &HashMap::new(),
            None,
            None,
            &DocxImportOptions::default(),
        );
        assert_eq!(mapped.sections.len(), 2);
    }

    #[test]
    fn a4_defaults_when_no_sect_pr() {
        let (doc, _) = run_map(&empty_doc(), None);
        assert_eq!(doc.sections.len(), 1);
        let sz = &doc.sections[0].layout.page_size;
        // A4: 595.28 × 841.89 pt
        assert!((sz.width.value() - PageSize::a4().width.value()).abs() < 0.1);
    }

    #[test]
    fn core_props_title_mapped() {
        let mut cp = loki_opc::CoreProperties::default();
        cp.title = Some("My Document".into());
        let doc = DocxDocument {
            body: DocxBody { children: vec![], final_sect_pr: None },
        };
        let (mapped, _) = map_document(
            &doc,
            &DocxStyles::default(),
            None,
            None,
            None,
            &HashMap::new(),
            &HashMap::new(),
            &HashMap::new(),
            &HashMap::new(),
            None,
            Some(&cp),
            &DocxImportOptions::default(),
        );
        assert_eq!(mapped.meta.title.as_deref(), Some("My Document"));
    }

    #[test]
    fn landscape_orientation_detected() {
        let mut sp = sect_pr_a4();
        if let Some(ref mut pg_sz) = sp.pg_sz {
            pg_sz.orient = Some("landscape".into());
        }
        let (doc, _) = run_map(&empty_doc(), Some(sp));
        assert_eq!(
            doc.sections[0].layout.orientation,
            PageOrientation::Landscape
        );
    }
}
