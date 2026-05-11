// Copyright 2026 AppThere Loki contributors
// SPDX-License-Identifier: Apache-2.0

mod helpers;

use std::io::Cursor;
use loki_doc_model::content::block::Block;
use loki_doc_model::content::inline::Inline;
use loki_doc_model::io::DocumentExport;
use loki_ooxml::DocxExport;
use loki_ooxml::docx::import::{DocxImportOptions, DocxImporter};

#[test]
fn export_round_trip_p1_features() {
    // ── 1. Import reference document ──────────────────────────────────────
    let ref_bytes = helpers::build_reference_docx();
    let doc_res = DocxImporter::new(DocxImportOptions::default())
        .run(&mut Cursor::new(&ref_bytes))
        .expect("import should succeed");
    let doc = doc_res.document;

    // Verify initial import state for P1 features
    assert!(doc.sections[0].layout.header.is_some(), "Initial import should have header");
    let all_blocks = doc.sections.iter().flat_map(|s| &s.blocks).collect::<Vec<_>>();
    
    // Verify first-line indent
    let has_first_line_indent = all_blocks.iter().any(|b| {
        if let Block::StyledPara(sp) = b {
            sp.direct_para_props.as_ref().and_then(|p| p.indent_first_line).is_some()
        } else {
            false
        }
    });
    assert!(has_first_line_indent, "Reference DOCX should have first-line indent");

    // Verify tab stops
    let has_tab_stops = all_blocks.iter().any(|b| {
        if let Block::StyledPara(sp) = b {
            sp.direct_para_props.as_ref().and_then(|p| p.tab_stops.as_ref()).is_some()
        } else {
            false
        }
    });
    assert!(has_tab_stops, "Reference DOCX should have tab stops");

    // Verify table row spans and background color
    let mut has_row_span = false;
    let mut has_cell_bg = false;
    for block in &all_blocks {
        if let Block::Table(t) = block {
            for body in &t.bodies {
                for row in &body.body_rows {
                    for cell in &row.cells {
                        if cell.row_span > 1 {
                            has_row_span = true;
                        }
                        if cell.props.background_color.is_some() {
                            has_cell_bg = true;
                        }
                    }
                }
            }
        }
    }
    assert!(has_row_span, "Reference DOCX should have row spans");
    assert!(has_cell_bg, "Reference DOCX should have cell background colors");

    // ── 2. Export back to DOCX ───────────────────────────────────────────
    let mut exported_bytes = Cursor::new(Vec::new());
    DocxExport::export(&doc, &mut exported_bytes, ())
        .expect("export should succeed");
    let exported_bytes = exported_bytes.into_inner();

    // ── 3. Re-import and verify ──────────────────────────────────────────
    let re_import = DocxImporter::new(DocxImportOptions::default())
        .run(&mut Cursor::new(&exported_bytes))
        .expect("re-import should succeed");
    
    let re_blocks = re_import.document.sections.iter().flat_map(|s| &s.blocks).collect::<Vec<_>>();

    // ── 4. Verify Paragraph Layout ───────────────────────────────────────
    let re_first_line = re_blocks.iter().any(|b| {
        if let Block::StyledPara(sp) = b {
            sp.direct_para_props.as_ref().and_then(|p| p.indent_first_line).is_some()
        } else {
            false
        }
    });
    assert!(re_first_line, "First-line indent should survive round-trip");

    let re_tab_stops = re_blocks.iter().any(|b| {
        if let Block::StyledPara(sp) = b {
            sp.direct_para_props.as_ref().and_then(|p| p.tab_stops.as_ref()).is_some()
        } else {
            false
        }
    });
    assert!(re_tab_stops, "Tab stops should survive round-trip");

    // ── 5. Verify Table Structural Fidelity ──────────────────────────────
    let mut re_row_span = false;
    let mut re_cell_bg = false;
    for block in &re_blocks {
        if let Block::Table(t) = block {
            for body in &t.bodies {
                for row in &body.body_rows {
                    for cell in &row.cells {
                        if cell.row_span > 1 {
                            re_row_span = true;
                        }
                        if cell.props.background_color.is_some() {
                            re_cell_bg = true;
                        }
                    }
                }
            }
        }
    }
    assert!(re_row_span, "Row spans should survive round-trip");
    assert!(re_cell_bg, "Cell background colors should survive round-trip");

    // ── 6. Verify Headers/Footers ────────────────────────────────────────
    let re_layout = &re_import.document.sections[0].layout;
    
    let has_header = re_layout.header.as_ref().is_some_and(|h| {
        h.blocks.iter().any(|b| {
            let inlines = match b {
                Block::Para(i) => Some(i),
                Block::StyledPara(sp) => Some(&sp.inlines),
                _ => None,
            };
            inlines.is_some_and(|inlines| {
                inlines.iter().any(|i| {
                    if let Inline::Str(s) = i {
                        s == "Test Document Header"
                    } else {
                        false
                    }
                })
            })
        })
    });
    assert!(has_header, "Default header should survive round-trip");

    let has_first_header = re_layout.header_first.as_ref().is_some_and(|h| {
        h.blocks.iter().any(|b| {
            let inlines = match b {
                Block::Para(i) => Some(i),
                Block::StyledPara(sp) => Some(&sp.inlines),
                _ => None,
            };
            inlines.is_some_and(|inlines| {
                inlines.iter().any(|i| {
                    if let Inline::Str(s) = i {
                        s == "First Page Header"
                    } else {
                        false
                    }
                })
            })
        })
    });
    assert!(has_first_header, "First page header should survive round-trip");
}
