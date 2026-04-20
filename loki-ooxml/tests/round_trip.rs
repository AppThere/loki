// Copyright 2026 AppThere Loki contributors
// SPDX-License-Identifier: Apache-2.0

//! Integration smoke tests: reference DOCX → import → assert document shape.

mod helpers;

use std::io::Cursor;

use loki_doc_model::content::block::Block;
use loki_doc_model::content::inline::{Inline, NoteKind};
use loki_ooxml::docx::import::{DocxImporter, DocxImportOptions};

/// Import the reference DOCX and validate the high-level document shape.
///
/// Checks:
/// 1. Document has at least one block.
/// 2. At least one `StyledRun` has `direct_props.bold == Some(true)`.
/// 3. At least one `StyledParagraph` has `direct_para_props.list_id` set.
/// 4. First section page size is approximately A4 (595 × 842 pt, ±1 pt).
/// 5. At least one paragraph has `border_top` set (gap #6).
/// 6. At least one paragraph has two explicit tab stops (gap #7).
/// 7. At least one paragraph contains `Inline::Note(Footnote, _)` (gap #2).
/// 8. At least one paragraph contains `Inline::Field` with `kind == PageNumber` (gap #4).
/// 9. Final section has a default header populated (gap #5).
/// 10. Final section has a default footer populated (gap #5).
/// 11. Final section has `header_first` set (`title_page = true`, gap #5).
#[test]
fn import_reference_docx_smoke() {
    let bytes = helpers::build_reference_docx();
    let result = DocxImporter::new(DocxImportOptions::default())
        .run(Cursor::new(bytes))
        .expect("reference DOCX should import without error");

    let doc = &result.document;

    // ── 1. Non-empty content ────────────────────────────────────────────────
    let all_blocks: Vec<&Block> =
        doc.sections.iter().flat_map(|s| s.blocks.iter()).collect();
    assert!(!all_blocks.is_empty(), "document must contain at least one block");

    // ── 2. Bold run present ─────────────────────────────────────────────────
    let has_bold = all_blocks.iter().any(|b| block_has_bold_run(b));
    assert!(has_bold, "at least one StyledRun with bold=true must be present");

    // ── 3. List paragraph present ───────────────────────────────────────────
    let has_list = all_blocks.iter().any(|b| {
        if let Block::StyledPara(p) = b {
            p.direct_para_props
                .as_ref()
                .map_or(false, |pp| pp.list_id.is_some())
        } else {
            false
        }
    });
    assert!(has_list, "at least one paragraph with list_id set must be present");

    // ── 4. A4 page size ─────────────────────────────────────────────────────
    let page_size = &doc.sections[0].layout.page_size;
    let w = page_size.width.value();
    let h = page_size.height.value();
    assert!(
        (w - 595.0).abs() < 1.0,
        "A4 width expected ~595 pt, got {w:.2}"
    );
    assert!(
        (h - 842.0).abs() < 1.0,
        "A4 height expected ~842 pt, got {h:.2}"
    );

    // ── 5. Paragraph border present (gap #6) ────────────────────────────────
    let has_border = all_blocks.iter().any(|b| {
        if let Block::StyledPara(p) = b {
            p.direct_para_props
                .as_ref()
                .map_or(false, |pp| pp.border_top.is_some())
        } else {
            false
        }
    });
    assert!(has_border, "at least one paragraph with border_top must be present (gap #6)");

    // ── 6. Tab stops present (gap #7) ───────────────────────────────────────
    let has_tab_stops = all_blocks.iter().any(|b| {
        if let Block::StyledPara(p) = b {
            p.direct_para_props
                .as_ref()
                .map_or(false, |pp| {
                    pp.tab_stops.as_ref().map_or(false, |ts| ts.len() >= 2)
                })
        } else {
            false
        }
    });
    assert!(has_tab_stops, "at least one paragraph with ≥2 tab stops must be present (gap #7)");

    // ── 7. Footnote present (gap #2) ────────────────────────────────────────
    let has_footnote = all_blocks.iter().any(|b| block_has_footnote(b));
    assert!(has_footnote, "at least one paragraph with Inline::Note(Footnote) must be present (gap #2)");

    // ── 8. Field code present (gap #4) ──────────────────────────────────────
    let has_field = all_blocks.iter().any(|b| block_has_field(b));
    assert!(has_field, "at least one paragraph with Inline::Field must be present (gap #4)");

    // ── 9. Default header populated (gap #5) ────────────────────────────────
    let final_layout = &doc.sections.last().unwrap().layout;
    let hdr = final_layout.header.as_ref()
        .expect("final section must have a default header (gap #5)");
    assert!(!hdr.blocks.is_empty(), "default header must contain at least one block");

    // ── 10. Default footer populated (gap #5) ───────────────────────────────
    let ftr = final_layout.footer.as_ref()
        .expect("final section must have a default footer (gap #5)");
    assert!(!ftr.blocks.is_empty(), "default footer must contain at least one block");

    // ── 11. First-page header present because titlePg is set (gap #5) ───────
    let hdr_first = final_layout.header_first.as_ref()
        .expect("final section must have a first-page header (titlePg, gap #5)");
    assert!(!hdr_first.blocks.is_empty(), "first-page header must contain at least one block");
}

fn block_has_bold_run(block: &Block) -> bool {
    let inlines = match block {
        Block::StyledPara(p) => p.inlines.as_slice(),
        Block::Heading(_, _, inlines) => inlines.as_slice(),
        _ => return false,
    };
    inlines.iter().any(inline_is_bold_styled_run)
}

fn inline_is_bold_styled_run(inline: &Inline) -> bool {
    if let Inline::StyledRun(run) = inline {
        run.direct_props
            .as_ref()
            .map_or(false, |cp| cp.bold == Some(true))
    } else {
        false
    }
}

fn block_has_footnote(block: &Block) -> bool {
    let inlines = match block {
        Block::StyledPara(p) => p.inlines.as_slice(),
        Block::Heading(_, _, inlines) => inlines.as_slice(),
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

fn block_has_field(block: &Block) -> bool {
    let inlines = match block {
        Block::StyledPara(p) => p.inlines.as_slice(),
        Block::Heading(_, _, inlines) => inlines.as_slice(),
        _ => return false,
    };
    inlines_have_field(inlines)
}

fn inlines_have_field(inlines: &[Inline]) -> bool {
    inlines.iter().any(|i| match i {
        Inline::Field(_) => true,
        Inline::StyledRun(run) => inlines_have_field(&run.content),
        _ => false,
    })
}

/// Verify that the layout engine assigns header/footer items to pages after
/// import — specifically that the first page gets the first-page header and
/// subsequent pages get the default header (gap #5).
#[test]
fn layout_assigns_header_footer_per_page() {
    use loki_layout::{layout_document, FontResources, LayoutMode};

    let bytes = helpers::build_reference_docx();
    let result = DocxImporter::new(DocxImportOptions::default())
        .run(Cursor::new(bytes))
        .expect("reference DOCX should import without error");

    let mut resources = FontResources::default();
    let layout = layout_document(
        &mut resources,
        &result.document,
        LayoutMode::Paginated,
        1.0,
    );

    let loki_layout::DocumentLayout::Paginated(paginated) = layout else {
        panic!("expected paginated layout");
    };

    assert!(!paginated.pages.is_empty(), "layout should produce at least one page");

    // Page 1 gets the first-page header variant (titlePg=true, header_first is set).
    let p1 = &paginated.pages[0];
    assert!(
        !p1.header_items.is_empty(),
        "page 1 should have header items (first-page header variant)"
    );
    assert!(p1.header_height > 0.0, "page 1 header_height should be > 0");
    assert!(
        !p1.footer_items.is_empty(),
        "page 1 should have footer items (first-page footer variant)"
    );
    assert!(p1.footer_height > 0.0, "page 1 footer_height should be > 0");

    // All pages should have both header and footer items.
    for (i, page) in paginated.pages.iter().enumerate() {
        assert!(
            !page.header_items.is_empty(),
            "page {} should have header items",
            i + 1
        );
        assert!(
            !page.footer_items.is_empty(),
            "page {} should have footer items",
            i + 1
        );
    }
}
