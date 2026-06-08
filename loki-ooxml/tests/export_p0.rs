// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 AppThere Loki contributors

//! Integration tests for P0 export features: Hyperlinks, Images, and Footnotes.

mod helpers;

use std::io::Cursor;

use loki_doc_model::content::block::Block;
use loki_doc_model::content::inline::{Inline, NoteKind};
use loki_doc_model::io::DocumentExport;
use loki_ooxml::DocxExport;
use loki_ooxml::docx::import::{DocxImportOptions, DocxImporter};

#[test]
fn export_round_trip_p0_features() {
    // ── 1. Import reference DOCX (contains links, images, footnotes) ─────
    let ref_bytes = helpers::build_reference_docx();
    let ref_import = DocxImporter::new(DocxImportOptions::default())
        .run(Cursor::new(ref_bytes))
        .expect("reference DOCX should import");
    let doc = ref_import.document;
    let mut ref_has_image = false;
    for section in &doc.sections {
        for block in &section.blocks {
            if block_has_image(block) {
                ref_has_image = true;
            }
        }
    }
    assert!(
        ref_has_image,
        "Reference DOCX should have an image after import"
    );

    // ── 2. Export back to DOCX ───────────────────────────────────────────
    let mut exported_bytes = Cursor::new(Vec::new());
    DocxExport::export(&doc, &mut exported_bytes, ()).expect("export should succeed");
    let exported_bytes = exported_bytes.into_inner();

    // ── 3. Re-import and verify ──────────────────────────────────────────
    let re_import = DocxImporter::new(DocxImportOptions::default())
        .run(Cursor::new(exported_bytes))
        .expect("exported DOCX should re-import");

    if !re_import.warnings.is_empty() {
        println!("RE-IMPORT WARNINGS: {:?}", re_import.warnings);
    }
    let re_doc = re_import.document;

    let all_blocks: Vec<&Block> = re_doc
        .sections
        .iter()
        .flat_map(|s| s.blocks.iter())
        .collect();

    // ── 4. Verify Hyperlinks ─────────────────────────────────────────────
    let has_link = all_blocks.iter().any(|b| block_has_link(b));
    assert!(has_link, "Hyperlink should survive round-trip");

    // ── 5. Verify Images ─────────────────────────────────────────────────
    let has_image = all_blocks.iter().any(|b| block_has_image(b));
    assert!(has_image, "Image should survive round-trip");

    // ── 6. Verify Footnotes ──────────────────────────────────────────────
    let has_footnote = all_blocks.iter().any(|b| block_has_footnote(b));
    assert!(has_footnote, "Footnote should survive round-trip");
}

fn block_has_link(block: &Block) -> bool {
    let inlines = match block {
        Block::Para(inlines) | Block::Plain(inlines) => inlines,
        Block::StyledPara(sp) => &sp.inlines,
        Block::Heading(_, _, inlines) => inlines,
        _ => return false,
    };
    inlines_have_link(inlines)
}

fn inlines_have_link(inlines: &[Inline]) -> bool {
    inlines.iter().any(|i| match i {
        Inline::Link(_, _, _) => true,
        Inline::StyledRun(run) => inlines_have_link(&run.content),
        _ => false,
    })
}

fn block_has_image(block: &Block) -> bool {
    let inlines = match block {
        Block::Para(inlines) | Block::Plain(inlines) => inlines,
        Block::StyledPara(sp) => &sp.inlines,
        Block::Heading(_, _, inlines) => inlines,
        _ => return false,
    };
    inlines_have_image(inlines)
}

fn inlines_have_image(inlines: &[Inline]) -> bool {
    inlines.iter().any(|i| match i {
        Inline::Image(_, _, _) => true,
        Inline::StyledRun(run) => inlines_have_image(&run.content),
        _ => false,
    })
}

fn block_has_footnote(block: &Block) -> bool {
    let inlines = match block {
        Block::Para(inlines) | Block::Plain(inlines) => inlines,
        Block::StyledPara(sp) => &sp.inlines,
        Block::Heading(_, _, inlines) => inlines,
        _ => return false,
    };
    inlines_have_footnote(inlines)
}

fn inlines_have_footnote(inlines: &[Inline]) -> bool {
    inlines.iter().any(|i| match i {
        Inline::Note(NoteKind::Footnote, _) => true,
        Inline::StyledRun(run) => inlines_have_footnote(&run.content),
        _ => false,
    })
}
