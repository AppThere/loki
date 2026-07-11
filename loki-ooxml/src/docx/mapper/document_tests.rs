// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 AppThere Loki contributors

//! Unit tests for the DOCX document mapper (`super`). Extracted from document.rs (Phase 7.1 inline-test extraction).

use super::*;
use crate::docx::model::document::{DocxBody, DocxDocument};
use crate::docx::model::paragraph::{DocxPPr, DocxParagraph, DocxPgMar, DocxPgSz, DocxSectPr};
use crate::docx::model::styles::DocxStyles;
use loki_doc_model::layout::page::PageSize;

fn empty_doc() -> DocxDocument {
    DocxDocument {
        body: DocxBody {
            children: vec![],
            final_sect_pr: None,
        },
    }
}

#[test]
fn section_type_maps_to_section_start() {
    assert_eq!(
        map_section_start(Some("continuous")),
        SectionStart::Continuous
    );
    assert_eq!(map_section_start(Some("evenPage")), SectionStart::EvenPage);
    assert_eq!(map_section_start(Some("oddPage")), SectionStart::OddPage);
    assert_eq!(map_section_start(Some("nextPage")), SectionStart::NewPage);
    // Absent / unknown → the default (nextPage).
    assert_eq!(map_section_start(None), SectionStart::NewPage);
}

fn sect_pr_a4() -> DocxSectPr {
    DocxSectPr {
        pg_sz: Some(DocxPgSz {
            w: 11906,
            h: 16838,
            orient: None,
        }),
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
        cols: None,
        pg_num_fmt: None,
        pg_num_start: None,
        section_type: None,
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
    let _sect_pr = sect_pr_a4();
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
    let cp = loki_opc::CoreProperties {
        title: Some("My Document".into()),
        ..Default::default()
    };
    let doc = DocxDocument {
        body: DocxBody {
            children: vec![],
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
