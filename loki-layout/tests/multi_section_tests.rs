// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 AppThere Loki contributors

//! Multi-section layout: block indices must be **global** (document order across
//! every section). The editor and the `loro_mutation` layer address blocks by a
//! single flat index, so a hit-test / cursor position must resolve to the right
//! section's block.

use loki_doc_model::content::attr::NodeAttr;
use loki_doc_model::content::block::{Block, StyledParagraph};
use loki_doc_model::content::inline::Inline;
use loki_doc_model::document::Document;
use loki_doc_model::layout::Section;

use loki_layout::{DocumentLayout, FontResources, LayoutMode, LayoutOptions, layout_document};

fn resources() -> FontResources {
    let mut r = FontResources::new();
    for p in [
        "/usr/share/fonts/truetype/liberation/LiberationSans-Regular.ttf",
        "/usr/share/fonts/truetype/dejavu/DejaVuSans.ttf",
    ] {
        if let Ok(data) = std::fs::read(p) {
            r.register_font(data);
            break;
        }
    }
    r
}

fn section(texts: &[&str]) -> Section {
    let mut s = Section::new();
    for t in texts {
        s.blocks.push(Block::StyledPara(StyledParagraph {
            style_id: None,
            direct_para_props: None,
            direct_char_props: None,
            inlines: vec![Inline::Str((*t).into())],
            attr: NodeAttr::default(),
        }));
    }
    s
}

#[test]
fn block_index_is_global_across_sections() {
    let mut doc = Document::new();
    doc.sections = vec![section(&["a0", "a1"]), section(&["b0", "b1", "b2"])];

    let mut r = resources();
    let layout = layout_document(
        &mut r,
        &doc,
        LayoutMode::Reflow {
            available_width: 600.0,
        },
        1.0,
        &LayoutOptions {
            preserve_for_editing: true,
        },
    );

    let DocumentLayout::Continuous(cl) = layout else {
        panic!("Reflow mode must yield a Continuous layout");
    };

    // 2 blocks in section 0 (global indices 0, 1), 3 in section 1 (global 2, 3,
    // 4). Without the global offset these would be [0, 1, 0, 1, 2] and section-1
    // edits would hit section 0.
    let indices: Vec<usize> = cl.paragraphs.iter().map(|p| p.block_index).collect();
    assert_eq!(
        indices,
        vec![0, 1, 2, 3, 4],
        "block indices must be global (cumulative) across sections"
    );
}
