// Copyright 2024-2026 AppThere
//
// Licensed under the Apache License, Version 2.0 (the "License");
// you may not use this file except in compliance with the License.
// You may obtain a copy of the License at
//
//     http://www.apache.org/licenses/LICENSE-2.0
//
// Unless required by applicable law or agreed to in writing, software
// distributed under the License is distributed on an "AS IS" BASIS,
// WITHOUT WARRANTIES OR CONDITIONS OF ANY KIND, either express or implied.
// See the License for the specific language governing permissions and
// limitations under the License.

//! Top-level document mapper: orchestrates all sub-mappers and produces a
//! [`loki_doc_model::Document`].

use std::collections::HashMap;

use loki_doc_model::content::attr::ExtensionBag;
use loki_doc_model::content::block::Block;
use loki_doc_model::document::Document;
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
use crate::docx::model::styles::{DocxTableModel, DocxStyles};
use crate::error::OoxmlWarning;

use super::numbering::map_numbering;
use super::styles::map_styles;

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

// ── Stub content mappers ──────────────────────────────────────────────────────

/// Maps a `w:p` paragraph to zero or more [`Block`]s.
///
/// **Implementation pending Session B.**  Currently returns an empty
/// `Vec` so that the section-splitting logic and integration tests can
/// run without panicking. The full implementation will produce
/// `Block::Paragraph`, `Block::Heading`, and `Block::StyledParagraph`
/// variants populated from the paragraph's runs and properties.
#[allow(unused_variables)]
pub(crate) fn map_paragraph(p: &DocxParagraph, ctx: &mut MappingContext<'_>) -> Vec<Block> {
    // TODO: implement in Session B
    Vec::new()
}

/// Maps a `w:tbl` table to a single [`Block`].
///
/// **Implementation pending Session B.**  Currently returns
/// [`Block::HorizontalRule`] as a placeholder marker so callers can
/// detect that a table was present.
#[allow(unused_variables)]
pub(crate) fn map_table(t: &DocxTableModel, ctx: &mut MappingContext<'_>) -> Block {
    // TODO: implement in Session B
    Block::HorizontalRule
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
            width: Points::new(pg_sz.w as f64 / 20.0),
            height: Points::new(pg_sz.h as f64 / 20.0),
        };
        layout.orientation = if pg_sz.orient.as_deref() == Some("landscape") {
            PageOrientation::Landscape
        } else {
            PageOrientation::Portrait
        };
    }

    if let Some(ref pg_mar) = sp.pg_mar {
        layout.margins = PageMargins {
            top: Points::new(pg_mar.top as f64 / 20.0),
            bottom: Points::new(pg_mar.bottom as f64 / 20.0),
            left: Points::new(pg_mar.left as f64 / 20.0),
            right: Points::new(pg_mar.right as f64 / 20.0),
            header: Points::new(pg_mar.header as f64 / 20.0),
            footer: Points::new(pg_mar.footer as f64 / 20.0),
            gutter: Points::new(pg_mar.gutter as f64 / 20.0),
        };
    }

    layout
}

// ── Note pre-processing ───────────────────────────────────────────────────────

/// Pre-processes a notes part into a `HashMap<id, Vec<Block>>`.
///
/// Only `Normal`-type notes are included; separators are skipped.
/// Full paragraph mapping is deferred to Session B — each note's
/// content is stored as an empty `Vec<Block>` until then.
fn preprocess_notes(notes: Option<&DocxNotes>) -> HashMap<i32, Vec<Block>> {
    let Some(notes) = notes else {
        return HashMap::new();
    };
    notes
        .notes
        .iter()
        .filter(|n| n.note_type == DocxNoteType::Normal)
        .map(|n| (n.id, Vec::new()))
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
pub(crate) fn map_document(
    doc: &DocxDocument,
    raw_styles: &DocxStyles,
    raw_numbering: Option<&DocxNumbering>,
    raw_footnotes: Option<&DocxNotes>,
    raw_endnotes: Option<&DocxNotes>,
    images: HashMap<String, PartData>,
    hyperlinks: HashMap<String, String>,
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
    let footnote_map = preprocess_notes(raw_footnotes);
    let endnote_map = preprocess_notes(raw_endnotes);

    // ── 4+5. Section-split body walk ───────────────────────────────────────
    let mut ctx = MappingContext {
        styles: &catalog,
        footnotes: &footnote_map,
        endnotes: &endnote_map,
        hyperlinks: &hyperlinks,
        images: &images,
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
                    let layout = map_page_layout(Some(sp));
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
    let final_layout = map_page_layout(doc.body.final_sect_pr.as_ref());
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
    use crate::docx::model::paragraph::{DocxPPr, DocxPgMar, DocxPgSz, DocxSectPr};
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
            HashMap::new(),
            HashMap::new(),
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
            HashMap::new(),
            HashMap::new(),
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
            HashMap::new(),
            HashMap::new(),
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
