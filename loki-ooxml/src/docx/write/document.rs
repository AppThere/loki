// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 AppThere Loki contributors

//! `word/document.xml` serializer.
//!
//! Converts a sequence of [`Section`]s into OOXML body content.  All
//! Tier-3 block and inline variants are handled; images, footnotes, and
//! complex fields are serialized via their sibling `write/` modules.
//!
//! ECMA-376 §17.2 (document structure) and §17.3 (block-level content).

use quick_xml::Writer;

use loki_doc_model::content::block::Block;
use loki_doc_model::layout::page::PageLayout;
use loki_doc_model::layout::section::Section;
use loki_doc_model::style::catalog::StyleCatalog;
use loki_doc_model::style::props::char_props::CharProps;

use crate::docx::write::collector::ExportCollector;
use crate::docx::write::section::write_sect_pr;

#[path = "document_blocks.rs"]
mod blocks;
#[path = "document_drawing.rs"]
mod drawing;
#[path = "document_inlines.rs"]
mod inlines;
#[path = "document_para.rs"]
mod para;
#[path = "document_table.rs"]
mod table;

use blocks::{write_code_block, write_horizontal_rule, write_line_block, write_list_item};
pub(super) use inlines::write_text_run;
use para::{write_para, write_styled_para};

use crate::docx::write::xml::{
    NS_A, NS_PIC, NS_R, NS_W, NS_WP, write_decl, write_end, write_start,
};

/// Serializes all sections to `word/document.xml` bytes.
/// `collector` is populated with lists, links, images, and notes encountered.
pub(super) fn write_document_xml(
    sections: &[Section],
    _catalog: &StyleCatalog,
    collector: &mut ExportCollector,
) -> Vec<u8> {
    let mut out = Vec::new();
    let mut w = Writer::new(&mut out);
    let _ = write_decl(&mut w);

    let _ = write_start(
        &mut w,
        "w:document",
        &[
            ("xmlns:w", NS_W),
            ("xmlns:r", NS_R),
            ("xmlns:wp", NS_WP),
            ("xmlns:a", NS_A),
            ("xmlns:pic", NS_PIC),
            ("xmlns:m", crate::docx::omml::OMML_NS),
        ],
    );
    let _ = write_start(&mut w, "w:body", &[]);

    for (idx, section) in sections.iter().enumerate() {
        let is_last = idx + 1 == sections.len();
        write_blocks(&mut w, &section.blocks, collector, 0);

        // Emit w:sectPr (last section: child of w:body; earlier: in a final para).
        let layout = &section.layout;
        if is_last {
            write_sect_pr(&mut w, layout, collector);
        } else {
            let _ = write_start(&mut w, "w:p", &[]);
            let _ = write_start(&mut w, "w:pPr", &[]);
            write_sect_pr(&mut w, layout, collector);
            let _ = write_end(&mut w, "w:pPr");
            let _ = write_end(&mut w, "w:p");
        }
    }

    if sections.is_empty() {
        // Always emit at least one empty paragraph and a sectPr.
        let _ = write_start(&mut w, "w:p", &[]);
        let _ = write_end(&mut w, "w:p");
        write_sect_pr(&mut w, &PageLayout::default(), collector);
    }

    let _ = write_end(&mut w, "w:body");
    let _ = write_end(&mut w, "w:document");
    drop(w);
    out
}

/// Serializes header/footer blocks to `word/headerN.xml` or `word/footerN.xml`.
pub(super) fn write_header_footer_xml(
    blocks: &[Block],
    collector: &mut ExportCollector,
    is_header: bool,
) -> Vec<u8> {
    let mut out = Vec::new();
    let mut w = Writer::new(&mut out);
    let _ = write_decl(&mut w);

    let tag = if is_header { "w:hdr" } else { "w:ftr" };
    let _ = write_start(
        &mut w,
        tag,
        &[
            ("xmlns:w", NS_W),
            ("xmlns:r", NS_R),
            ("xmlns:wp", NS_WP),
            ("xmlns:a", NS_A),
            ("xmlns:pic", NS_PIC),
            ("xmlns:m", crate::docx::omml::OMML_NS),
        ],
    );

    write_blocks(&mut w, blocks, collector, 0);

    let _ = write_end(&mut w, tag);
    drop(w);
    out
}

// ── Block dispatch ───────────────────────────────────────────────────────────

/// Recursively writes a slice of blocks.  `list_level` tracks nesting depth
/// for nested lists (currently always 0 since we only support ilvl=0).
pub(super) fn write_blocks<W: std::io::Write>(
    w: &mut Writer<W>,
    blocks: &[Block],
    collector: &mut ExportCollector,
    list_level: u8,
) {
    for block in blocks {
        write_block(w, block, collector, list_level);
    }
}

fn write_block<W: std::io::Write>(
    w: &mut Writer<W>,
    block: &Block,
    collector: &mut ExportCollector,
    _list_level: u8,
) {
    match block {
        Block::Para(inlines) | Block::Plain(inlines) => {
            write_para(w, None, None, inlines, collector);
        }
        Block::StyledPara(sp) => {
            write_styled_para(w, sp, collector);
        }
        Block::Heading(level, _, inlines) => {
            let style_id = format!("Heading{level}");
            write_para(w, Some(&style_id), None, inlines, collector);
        }
        Block::BulletList(items) => {
            let num_id = collector.num_state.register_bullet();
            for item_blocks in items {
                write_list_item(w, item_blocks, num_id, 0, collector);
            }
        }
        Block::OrderedList(attrs, items) => {
            let num_id = collector.num_state.register_ordered(attrs);
            for item_blocks in items {
                write_list_item(w, item_blocks, num_id, 0, collector);
            }
        }
        Block::Table(tbl) => {
            table::write_table(w, tbl, collector);
        }
        Block::HorizontalRule => {
            write_horizontal_rule(w);
        }
        Block::CodeBlock(_, code) => {
            write_code_block(w, code, collector);
        }
        Block::BlockQuote(blocks) | Block::Div(_, blocks) => {
            write_blocks(w, blocks, collector, 0);
        }
        Block::LineBlock(lines) => {
            write_line_block(w, lines, collector);
        }
        Block::DefinitionList(items) => {
            for (term, defs) in items {
                write_para(w, None, None, term, collector);
                for def_blocks in defs {
                    write_blocks(w, def_blocks, collector, 0);
                }
            }
        }
        Block::TableOfContents(toc) => {
            write_blocks(w, &toc.body, collector, 0);
        }
        Block::Index(idx) => {
            write_blocks(w, &idx.body, collector, 0);
        }
        Block::Figure(_, caption, blocks) => {
            // Figures often contain an image block.
            write_blocks(w, blocks, collector, 0);
            // And emit the caption as a paragraph.
            if !caption.full.is_empty() {
                write_blocks(w, &caption.full, collector, 0);
            }
        }
        _ => {}
    }
}

// ── Inline dispatch ──────────────────────────────────────────────────────────

/// Accumulated run formatting inherited from inline wrappers.
#[allow(clippy::struct_excessive_bools)] // Pre-existing pattern — structural refactor deferred
#[derive(Default, Clone)]
pub(super) struct RunProps {
    bold: bool,
    italic: bool,
    underline: bool,
    strikethrough: bool,
    superscript: bool,
    subscript: bool,
    small_caps: bool,
    code: bool,
    char_style: Option<String>,
    direct: Option<CharProps>,
}
