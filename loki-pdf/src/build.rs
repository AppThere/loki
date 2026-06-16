// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 AppThere Loki contributors

//! Assembles a complete PDF/X document from a paginated layout.
//!
//! Object order: content streams are built first (collecting every used face),
//! then the catalog, page tree, pages, embedded fonts, document info, the XMP
//! packet, and the optional ICC output profile.

use std::collections::hash_map::DefaultHasher;
use std::hash::{Hash, Hasher};

use chrono::Utc;
use loki_doc_model::Document;
use loki_layout::PaginatedLayout;
use pdf_writer::types::OutputIntentSubtype;
use pdf_writer::{Finish, Name, Pdf, Rect, Ref, TextStr};

use crate::error::PdfError;
use crate::fonts::FontBank;
use crate::metadata::{build_xmp, write_info};
use crate::options::PdfXOptions;
use crate::page::render_page_content;

/// Builds the PDF/X byte stream for `layout`.
pub fn write_document(
    layout: &PaginatedLayout,
    doc: &Document,
    options: &PdfXOptions,
) -> Result<Vec<u8>, PdfError> {
    // ── Pass 1: build content streams, collecting the faces they use. ────────
    let mut bank = FontBank::new();
    let contents: Vec<Vec<u8>> = layout
        .pages
        .iter()
        .map(|page| render_page_content(page, &mut bank))
        .collect();

    // ── Allocate indirect references. ────────────────────────────────────────
    let catalog_id = Ref::new(1);
    let page_tree_id = Ref::new(2);
    let mut next = 3;
    let mut alloc = || {
        let r = Ref::new(next);
        next += 1;
        r
    };
    let page_refs: Vec<(Ref, Ref)> = layout.pages.iter().map(|_| (alloc(), alloc())).collect();
    let xmp_id = alloc();
    let info_id = alloc();
    let icc_id = options.output_intent.icc_profile.as_ref().map(|_| alloc());
    let face_refs = bank.allocate_refs(&mut next);

    // ── Document setup. ──────────────────────────────────────────────────────
    let (major, minor) = options.level.pdf_version();
    let mut pdf = Pdf::new();
    pdf.set_version(major, minor);
    let (id_a, id_b) = file_id(doc, layout.pages.len());
    pdf.set_file_id((id_a, id_b));

    write_catalog(&mut pdf, catalog_id, page_tree_id, xmp_id, options, icc_id);

    // ── Page tree + pages. ───────────────────────────────────────────────────
    pdf.pages(page_tree_id)
        .kids(page_refs.iter().map(|(p, _)| *p))
        .count(page_refs.len() as i32);

    for ((page, (page_id, content_id)), bytes) in layout.pages.iter().zip(&page_refs).zip(&contents)
    {
        write_page(
            &mut pdf,
            *page_id,
            page_tree_id,
            page.page_size.width,
            page.page_size.height,
            *content_id,
            &bank,
            &face_refs,
        );
        pdf.stream(*content_id, bytes);
    }

    // ── Fonts, info, XMP, ICC. ───────────────────────────────────────────────
    bank.embed(&mut pdf, &face_refs)?;

    let modified = doc.meta.modified.unwrap_or_else(Utc::now);
    let created = doc.meta.created.unwrap_or(modified);
    let title = options
        .title
        .clone()
        .or_else(|| doc.meta.title.clone())
        .unwrap_or_else(|| "Untitled".to_string());
    write_info(
        &mut pdf,
        info_id,
        &doc.meta,
        &title,
        options.level,
        created,
        modified,
    );

    let xmp = build_xmp(&doc.meta, &title, options.level, created, modified);
    pdf.stream(xmp_id, xmp.as_bytes())
        .pair(Name(b"Type"), Name(b"Metadata"))
        .pair(Name(b"Subtype"), Name(b"XML"));

    if let (Some(icc_ref), Some(profile)) = (icc_id, options.output_intent.icc_profile.as_ref()) {
        pdf.icc_profile(icc_ref, profile)
            .n(4)
            .alternate_name(Name(b"DeviceCMYK"));
    }

    Ok(pdf.finish())
}

fn write_catalog(
    pdf: &mut Pdf,
    catalog_id: Ref,
    page_tree_id: Ref,
    xmp_id: Ref,
    options: &PdfXOptions,
    icc_id: Option<Ref>,
) {
    let intent = &options.output_intent;
    let (major, minor) = options.level.pdf_version();
    let mut catalog = pdf.catalog(catalog_id);
    catalog.pages(page_tree_id);
    catalog.metadata(xmp_id);
    catalog.version(major, minor);
    catalog.lang(TextStr("en"));
    {
        let mut intents = catalog.output_intents();
        let mut oi = intents.push();
        oi.subtype(OutputIntentSubtype::PDFX);
        oi.output_condition_identifier(TextStr(&intent.condition_identifier));
        if let Some(c) = intent.condition.as_deref() {
            oi.output_condition(TextStr(c));
        }
        if let Some(r) = intent.registry_name.as_deref() {
            oi.registry_name(TextStr(r));
        }
        if let Some(i) = intent.info.as_deref() {
            oi.info(TextStr(i));
        }
        if let Some(icc) = icc_id {
            oi.dest_output_profile(icc);
        }
        oi.finish();
        intents.finish();
    }
    catalog.finish();
}

#[allow(clippy::too_many_arguments)]
fn write_page(
    pdf: &mut Pdf,
    page_id: Ref,
    page_tree_id: Ref,
    width: f32,
    height: f32,
    content_id: Ref,
    bank: &FontBank,
    face_refs: &[crate::fonts::FaceRefs],
) {
    let mut page = pdf.page(page_id);
    page.parent(page_tree_id);
    page.media_box(Rect::new(0.0, 0.0, width, height));
    page.contents(content_id);
    {
        let mut resources = page.resources();
        let mut fonts = resources.fonts();
        for (face, fr) in bank.faces().iter().zip(face_refs) {
            fonts.pair(Name(face.resource.as_bytes()), fr.type0);
        }
        fonts.finish();
        resources.finish();
    }
    page.finish();
}

/// Derives a stable 16-byte trailer `/ID` pair (required by PDF/X) from the
/// document identity.
fn file_id(doc: &Document, page_count: usize) -> (Vec<u8>, Vec<u8>) {
    let mut h = DefaultHasher::new();
    doc.meta.title.hash(&mut h);
    doc.meta.creator.hash(&mut h);
    page_count.hash(&mut h);
    let seed = Hasher::finish(&h);
    let bytes: Vec<u8> = (0..16u8)
        .map(|i| (seed.rotate_left(u32::from(i) * 4) & 0xff) as u8)
        .collect();
    (bytes.clone(), bytes)
}
