// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 AppThere Loki contributors

//! Non-paragraph block writers, split out of `document.rs` for the 300-line
//! ceiling: code blocks, horizontal rules, line blocks, and list items. All
//! are re-exported into the parent for the `write_block` dispatcher; the
//! recursive `write_list_item` reaches back for `super::{write_block,
//! write_para}`.

use quick_xml::Writer;

use loki_doc_model::content::block::Block;
use loki_doc_model::content::inline::Inline;

use crate::docx::write::collector::ExportCollector;
use crate::docx::write::xml::{write_empty, write_end, write_start, wval};

use super::inlines::write_inlines;
use super::{RunProps, write_block, write_para, write_text_run};

pub(super) fn write_code_block<W: std::io::Write>(
    w: &mut Writer<W>,
    code: &str,
    _collector: &mut ExportCollector,
) {
    let _ = write_start(w, "w:p", &[]);
    let _ = write_start(w, "w:pPr", &[]);
    let _ = write_empty(w, "w:pStyle", &wval("Code"));
    let _ = write_end(w, "w:pPr");
    write_text_run(
        w,
        code,
        &RunProps {
            code: true,
            ..Default::default()
        },
    );
    let _ = write_end(w, "w:p");
}

pub(super) fn write_horizontal_rule<W: std::io::Write>(w: &mut Writer<W>) {
    let _ = write_start(w, "w:p", &[]);
    let _ = write_start(w, "w:pPr", &[]);
    let _ = write_start(w, "w:pBdr", &[]);
    let _ = write_empty(
        w,
        "w:bottom",
        &[
            ("w:val", "single"),
            ("w:sz", "6"),
            ("w:space", "1"),
            ("w:color", "auto"),
        ],
    );
    let _ = write_end(w, "w:pBdr");
    let _ = write_end(w, "w:pPr");
    let _ = write_end(w, "w:p");
}

pub(super) fn write_line_block<W: std::io::Write>(
    w: &mut Writer<W>,
    lines: &[Vec<Inline>],
    collector: &mut ExportCollector,
) {
    for (idx, line) in lines.iter().enumerate() {
        let _ = write_start(w, "w:p", &[]);
        write_inlines(w, line, &RunProps::default(), collector);
        if idx + 1 < lines.len() {
            // Add a line break run between lines (last line gets its own paragraph).
            let _ = write_start(w, "w:r", &[]);
            let _ = write_empty(w, "w:br", &[]);
            let _ = write_end(w, "w:r");
        }
        let _ = write_end(w, "w:p");
    }
}

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
