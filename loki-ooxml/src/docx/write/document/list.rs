// SPDX-License-Identifier: Apache-2.0
// Copyright (C) 2025 AppThere contributors

//! List-item serializer for `word/document.xml`.

use quick_xml::Writer;

use loki_doc_model::content::block::Block;

use crate::docx::write::collector::ExportCollector;
use crate::docx::write::xml::{write_empty, write_end, write_start, wval};

use super::inline::{RunProps, write_inlines};
use super::para::write_para;
use super::write_block;

/// Writes a list item: first block becomes a paragraph with numPr;
/// subsequent blocks are written recursively (without numPr).
pub(super) fn write_list_item<W: std::io::Write>(
    w: &mut Writer<W>,
    blocks: &[Block],
    num_id: u32,
    ilvl: u8,
    collector: &mut ExportCollector,
) {
    let mut first = true;
    for block in blocks {
        if first {
            first = false;
            match block {
                Block::Para(inlines) | Block::Plain(inlines) => {
                    write_para(w, None, Some((num_id, ilvl)), inlines, collector);
                }
                Block::StyledPara(sp) => {
                    // Inject numPr into styled para.
                    let _ = write_start(w, "w:p", &[]);
                    let _ = write_start(w, "w:pPr", &[]);
                    if let Some(ref sid) = sp.style_id {
                        let _ = write_empty(w, "w:pStyle", &wval(sid.as_str()));
                    }
                    let num_id_s = num_id.to_string();
                    let ilvl_s = ilvl.to_string();
                    let _ = write_start(w, "w:numPr", &[]);
                    let _ = write_empty(w, "w:ilvl", &wval(&ilvl_s));
                    let _ = write_empty(w, "w:numId", &wval(&num_id_s));
                    let _ = write_end(w, "w:numPr");
                    let _ = write_end(w, "w:pPr");
                    write_inlines(w, &sp.inlines, &RunProps::default(), collector);
                    let _ = write_end(w, "w:p");
                }
                // For non-para first block, emit an empty list para + recurse.
                other => {
                    write_para(w, None, Some((num_id, ilvl)), &[], collector);
                    write_block(w, other, collector, ilvl);
                }
            }
        } else {
            write_block(w, block, collector, ilvl);
        }
    }
    if blocks.is_empty() {
        write_para(w, None, Some((num_id, ilvl)), &[], collector);
    }
}
