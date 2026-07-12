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

use loki_doc_model::content::block::{Block, StyledParagraph};
use loki_doc_model::content::inline::Inline;
use loki_doc_model::layout::page::PageLayout;
use loki_doc_model::layout::section::Section;
use loki_doc_model::style::catalog::StyleCatalog;
use loki_doc_model::style::props::char_props::CharProps;

use crate::docx::write::collector::ExportCollector;
use crate::docx::write::run_props::emit_char_props;
use crate::docx::write::section::write_sect_pr;

#[path = "document_drawing.rs"]
mod drawing;
#[path = "document_inlines.rs"]
mod inlines;
#[path = "document_table.rs"]
mod table;

use inlines::write_inlines;
pub(super) use inlines::write_text_run;

use crate::docx::write::xml::{
    NS_A, NS_PIC, NS_R, NS_W, NS_WP, pts_to_twips, write_decl, write_empty, write_end, write_start,
    wval,
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

// ── Paragraph helpers ────────────────────────────────────────────────────────

/// Writes `<w:p>` with optional `w:pStyle` and optional `w:numPr`.
fn write_para<W: std::io::Write>(
    w: &mut Writer<W>,
    style_id: Option<&str>,
    num_pr: Option<(u32, u8)>, // (numId, ilvl)
    inlines: &[Inline],
    collector: &mut ExportCollector,
) {
    let _ = write_start(w, "w:p", &[]);

    let has_ppr = style_id.is_some() || num_pr.is_some();
    if has_ppr {
        let _ = write_start(w, "w:pPr", &[]);
        if let Some(sid) = style_id {
            let _ = write_empty(w, "w:pStyle", &wval(sid));
        }
        if let Some((num_id, ilvl)) = num_pr {
            let num_id_s = num_id.to_string();
            let ilvl_s = ilvl.to_string();
            let _ = write_start(w, "w:numPr", &[]);
            let _ = write_empty(w, "w:ilvl", &wval(&ilvl_s));
            let _ = write_empty(w, "w:numId", &wval(&num_id_s));
            let _ = write_end(w, "w:numPr");
        }
        let _ = write_end(w, "w:pPr");
    }

    write_inlines(w, inlines, &RunProps::default(), collector);
    let _ = write_end(w, "w:p");
}

#[allow(clippy::similar_names)] // has_pp / has_cp / has_style — pre-existing naming
fn write_styled_para<W: std::io::Write>(
    w: &mut Writer<W>,
    sp: &StyledParagraph,
    collector: &mut ExportCollector,
) {
    let _ = write_start(w, "w:p", &[]);

    let has_style = sp.style_id.is_some();
    let has_pp = sp.direct_para_props.is_some();
    let has_cp = sp.direct_char_props.is_some();

    if has_style || has_pp || has_cp {
        let _ = write_start(w, "w:pPr", &[]);
        if let Some(ref sid) = sp.style_id {
            let _ = write_empty(w, "w:pStyle", &wval(sid.as_str()));
        }
        if let Some(ref pp) = sp.direct_para_props {
            // Emit para prop children inline (not a nested w:pPr).
            write_para_props_inline(w, pp);
        }
        if let Some(ref cp) = sp.direct_char_props {
            let _ = write_start(w, "w:rPr", &[]);
            // A tracked deletion of the paragraph mark rides its rPr (w:del).
            super::revision::write_mark_del(w, cp.revision.as_ref());
            emit_char_props(w, cp);
            let _ = write_end(w, "w:rPr");
        }
        let _ = write_end(w, "w:pPr");
    }

    write_inlines(w, &sp.inlines, &RunProps::default(), collector);
    let _ = write_end(w, "w:p");
}

/// Emits the children of `w:pPr` from a [`ParaProps`] (no wrapper element).
#[allow(clippy::too_many_lines, unused_assignments)] // Pre-existing pattern — structural refactor deferred
fn write_para_props_inline<W: std::io::Write>(
    w: &mut Writer<W>,
    pp: &loki_doc_model::style::props::para_props::ParaProps,
) {
    use loki_doc_model::style::props::para_props::ParagraphAlignment;

    if let Some(align) = pp.alignment {
        let jc = match align {
            ParagraphAlignment::Right => "right",
            ParagraphAlignment::Center => "center",
            ParagraphAlignment::Justify => "both",
            _ => "left",
        };
        let _ = write_empty(w, "w:jc", &wval(jc));
    }

    let has_ind = pp.indent_start.is_some()
        || pp.indent_end.is_some()
        || pp.indent_hanging.is_some()
        || pp.indent_first_line.is_some();
    if has_ind {
        let left = pp
            .indent_start
            .map_or(0, |v| pts_to_twips(v.value()))
            .to_string();
        let right = pp
            .indent_end
            .map_or(0, |v| pts_to_twips(v.value()))
            .to_string();
        let hanging = pp
            .indent_hanging
            .map_or(0, |v| pts_to_twips(v.value()))
            .to_string();
        let first_line = pp
            .indent_first_line
            .map_or(0, |v| pts_to_twips(v.value()))
            .to_string();
        let mut attrs: Vec<(&str, &str)> = Vec::new();
        if pp.indent_start.is_some() {
            attrs.push(("w:left", &left));
        }
        if pp.indent_end.is_some() {
            attrs.push(("w:right", &right));
        }
        if pp.indent_hanging.is_some() {
            attrs.push(("w:hanging", &hanging));
        }
        if pp.indent_first_line.is_some() {
            attrs.push(("w:firstLine", &first_line));
        }
        if !attrs.is_empty() {
            let _ = write_empty(w, "w:ind", &attrs);
        }
    }

    if let Some(tabs) = &pp.tab_stops {
        let _ = write_start(w, "w:tabs", &[]);
        for ts in tabs {
            use loki_doc_model::style::props::tab_stop::{TabAlignment, TabLeader};
            let val = match ts.alignment {
                TabAlignment::Center => "center",
                TabAlignment::Right => "right",
                TabAlignment::Decimal => "decimal",
                TabAlignment::Clear => "clear",
                _ => "left",
            };
            let pos = pts_to_twips(ts.position.value()).to_string();
            let leader = match ts.leader {
                TabLeader::Dot => "dot",
                TabLeader::Dash => "dash",
                TabLeader::Underscore => "underscore",
                TabLeader::Heavy => "heavy",
                TabLeader::MiddleDot => "middleDot",
                _ => "none",
            };
            let _ = write_empty(
                w,
                "w:tab",
                &[("w:val", val), ("w:pos", &pos), ("w:leader", leader)],
            );
        }
        let _ = write_end(w, "w:tabs");
    }

    if pp.space_before.is_some() || pp.space_after.is_some() || pp.line_height.is_some() {
        use loki_doc_model::style::props::para_props::{LineHeight, Spacing};
        let mut attrs: Vec<(&str, &str)> = Vec::new();

        #[allow(clippy::match_same_arms)] // Spacing is #[non_exhaustive]; wildcard required
        let before = pp.space_before.map(|v| match v {
            Spacing::Exact(pt) => pts_to_twips(pt.value()),
            Spacing::Percent(_) | _ => 0,
        });
        let before_s;
        if let Some(b) = before {
            before_s = b.to_string();
            attrs.push(("w:before", &before_s));
        }

        let after = pp.space_after.map(|v| match v {
            Spacing::Exact(pt) => pts_to_twips(pt.value()),
            Spacing::Percent(_) | _ => 0,
        });
        let after_s;
        if let Some(a) = after {
            after_s = a.to_string();
            attrs.push(("w:after", &after_s));
        }

        let mut line_s = String::new();
        if let Some(lh) = pp.line_height {
            match lh {
                LineHeight::Exact(pt) => {
                    line_s = pts_to_twips(pt.value()).to_string();
                    attrs.push(("w:line", &line_s));
                    attrs.push(("w:lineRule", "exact"));
                }
                LineHeight::AtLeast(pt) => {
                    line_s = pts_to_twips(pt.value()).to_string();
                    attrs.push(("w:line", &line_s));
                    attrs.push(("w:lineRule", "atLeast"));
                }
                LineHeight::Multiple(f) => {
                    line_s = (f * 2.4).round().to_string();
                    attrs.push(("w:line", &line_s));
                    attrs.push(("w:lineRule", "auto"));
                }
                _ => {}
            }
        }

        if !attrs.is_empty() {
            let _ = write_empty(w, "w:spacing", &attrs);
        }
    }
}

fn write_code_block<W: std::io::Write>(
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

fn write_horizontal_rule<W: std::io::Write>(w: &mut Writer<W>) {
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

fn write_line_block<W: std::io::Write>(
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
fn write_list_item<W: std::io::Write>(
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
