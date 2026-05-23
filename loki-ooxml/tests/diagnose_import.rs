// Copyright 2026 AppThere Loki contributors
// SPDX-License-Identifier: Apache-2.0

use std::fs::File;
use std::io::BufReader;

use loki_doc_model::content::block::Block;
use loki_doc_model::content::inline::Inline;
use loki_ooxml::docx::import::{DocxImportOptions, DocxImporter};
use loki_doc_model::style::props::char_props::CharProps;

#[test]
fn diagnose_loki_architecture_import() {
    let path = "tests/AppThere_Loki_Architecture.docx";
    if !std::path::Path::new(path).exists() {
        println!("Fixture not found at {}", path);
        return;
    }

    let file = File::open(path).unwrap();
    let result = DocxImporter::new(DocxImportOptions::default())
        .run(BufReader::new(file))
        .unwrap();
    let doc = result.document;

    println!("=== Style Catalog Character Styles ===");
    for id in doc.styles.character_styles.keys() {
        println!("  char_style: {}", id.as_str());
    }

    println!("=== Style Catalog Paragraph Styles ===");
    for id in doc.styles.paragraph_styles.keys() {
        let style = doc.styles.paragraph_styles.get(id).unwrap();
        println!("  para_style: {} (parent: {:?})", id.as_str(), style.parent);
    }

    println!("=== Fontique Calibri Lookup ===");
    let mut font_cx = parley::FontContext::new();
    if let Some(family_id) = font_cx.collection.family_id("Calibri") {
        if let Some(family_info) = font_cx.collection.family(family_id) {
            println!("Family name: {}", family_info.name());
            for (face_idx, font_info) in family_info.fonts().iter().enumerate() {
                let blob = font_info.load(Some(&mut font_cx.source_cache));
                let blob_len = blob.as_ref().map(|b| b.len()).unwrap_or(0);
                let blob_index = font_info.index();
                
                let mut mappings = Vec::new();
                let mut file_type = "Unknown".to_string();
                if let Some(b) = &blob {
                    if let Ok(file_ref) = read_fonts::FileRef::new(b.as_ref()) {
                        match file_ref {
                            read_fonts::FileRef::Font(_) => file_type = "Single Font".to_string(),
                            read_fonts::FileRef::Collection(c) => {
                                file_type = format!("Collection of {} fonts", c.len());
                            }
                        }
                    }
                    let charmap_index = font_info.charmap_index();
                    if let Some(charmap) = charmap_index.charmap(b.as_ref()) {
                        let text = "AppThere Loki Cross-Platform Office Suite";
                        for c in text.chars() {
                            if !mappings.iter().any(|(x, _)| *x == c) {
                                mappings.push((c, charmap.map(c)));
                            }
                        }
                    }
                    if let Ok(font_ref) = read_fonts::FontRef::from_index(b.as_ref(), blob_index) {
                        use read_fonts::TableProvider;
                        let mut tags: Vec<String> = font_ref.table_directory().table_records().iter().map(|r| {
                            let tag_bytes = r.tag().into_bytes();
                            String::from_utf8_lossy(&tag_bytes).to_string()
                        }).collect();
                        file_type = format!("{} tables: {:?}", file_type, tags);
                        if let Ok(fvar) = font_ref.fvar() {
                            if let Ok(axes) = fvar.axes() {
                                file_type = format!("{} with {} axes", file_type, axes.len());
                            }
                        }
                    }
                }

                println!(
                    "Resolved font: face_idx={}, blob_len={}, blob_index={}, file_type={}, weight={:?}, style={:?}, mappings={:?}",
                    face_idx,
                    blob_len,
                    blob_index,
                    file_type,
                    font_info.weight(),
                    font_info.style(),
                    mappings
                );
            }
        }
    } else {
        println!("Calibri family not found");
    }

    println!("=== Content Blocks ===");
    if let Some(section) = doc.first_section() {
        for (block_idx, block) in section.blocks.iter().enumerate().take(50) {
            println!("Block {}: {:?}", block_idx, block);
        }
    }
}
