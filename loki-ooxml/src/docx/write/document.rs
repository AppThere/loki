// Copyright 2026 AppThere Loki contributors
// SPDX-License-Identifier: Apache-2.0

//! `word/document.xml` serializer.
//!
//! Converts a sequence of [`Section`]s into OOXML body content.  All
//! Tier-3 block and inline variants are handled; Tier-4+ content (images,
//! footnotes, complex fields) is silently omitted.
//!
//! ECMA-376 §17.2 (document structure) and §17.3 (block-level content).

use quick_xml::Writer;

use loki_doc_model::content::block::{Block, StyledParagraph};
use loki_doc_model::content::inline::{Inline, NoteKind, StyledRun};
use loki_doc_model::content::table::core::Table;
use loki_doc_model::content::table::row::Cell;
use loki_doc_model::layout::page::PageLayout;
use loki_doc_model::layout::section::Section;
use loki_doc_model::style::catalog::StyleCatalog;
use loki_doc_model::style::props::char_props::CharProps;

use crate::docx::write::collector::ExportCollector;
use crate::docx::write::styles::emit_char_props;
use crate::docx::write::xml::{
    NS_A, NS_PIC, NS_R, NS_W, NS_WP, color_to_hex, pts_to_twips, write_decl, write_empty,
    write_end, write_start, wval,
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
        ],
    );
    let _ = write_start(&mut w, "w:body", &[]);

    for (idx, section) in sections.iter().enumerate() {
        let is_last = idx + 1 == sections.len();
        write_blocks(&mut w, &section.blocks, collector, 0);

        // Emit w:sectPr — for the last section it is a direct child of w:body;
        // for earlier sections it goes inside a final empty paragraph.
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

// ── Section properties ───────────────────────────────────────────────────────

fn write_sect_pr<W: std::io::Write>(
    w: &mut Writer<W>,
    layout: &PageLayout,
    collector: &mut ExportCollector,
) {
    let _ = write_start(w, "w:sectPr", &[]);

    if let Some(hf) = &layout.header {
        let r_id = collector.add_header_footer(hf.blocks.clone(), true);
        let _ = write_empty(
            w,
            "w:headerReference",
            &[("w:type", "default"), ("r:id", &r_id)],
        );
    }
    if let Some(hf) = &layout.header_first {
        let r_id = collector.add_header_footer(hf.blocks.clone(), true);
        let _ = write_empty(
            w,
            "w:headerReference",
            &[("w:type", "first"), ("r:id", &r_id)],
        );
    }
    if let Some(hf) = &layout.header_even {
        let r_id = collector.add_header_footer(hf.blocks.clone(), true);
        let _ = write_empty(
            w,
            "w:headerReference",
            &[("w:type", "even"), ("r:id", &r_id)],
        );
    }

    if let Some(hf) = &layout.footer {
        let r_id = collector.add_header_footer(hf.blocks.clone(), false);
        let _ = write_empty(
            w,
            "w:footerReference",
            &[("w:type", "default"), ("r:id", &r_id)],
        );
    }
    if let Some(hf) = &layout.footer_first {
        let r_id = collector.add_header_footer(hf.blocks.clone(), false);
        let _ = write_empty(
            w,
            "w:footerReference",
            &[("w:type", "first"), ("r:id", &r_id)],
        );
    }
    if let Some(hf) = &layout.footer_even {
        let r_id = collector.add_header_footer(hf.blocks.clone(), false);
        let _ = write_empty(
            w,
            "w:footerReference",
            &[("w:type", "even"), ("r:id", &r_id)],
        );
    }

    if layout.header_first.is_some()
        || layout.footer_first.is_some()
        || layout.header_even.is_some()
        || layout.footer_even.is_some()
    {
        let _ = write_empty(w, "w:titlePg", &[]);
    }

    let pw = pts_to_twips(layout.page_size.width.value()).to_string();
    let ph = pts_to_twips(layout.page_size.height.value()).to_string();
    let orient = match layout.orientation {
        loki_doc_model::layout::page::PageOrientation::Landscape => "landscape",
        _ => "portrait",
    };
    let _ = write_empty(
        w,
        "w:pgSz",
        &[("w:w", &pw), ("w:h", &ph), ("w:orient", orient)],
    );

    let mt = pts_to_twips(layout.margins.top.value()).to_string();
    let mb = pts_to_twips(layout.margins.bottom.value()).to_string();
    let ml = pts_to_twips(layout.margins.left.value()).to_string();
    let mr = pts_to_twips(layout.margins.right.value()).to_string();
    let mh = pts_to_twips(layout.margins.header.value()).to_string();
    let mf = pts_to_twips(layout.margins.footer.value()).to_string();
    let mg = pts_to_twips(layout.margins.gutter.value()).to_string();
    let _ = write_empty(
        w,
        "w:pgMar",
        &[
            ("w:top", &mt),
            ("w:right", &mr),
            ("w:bottom", &mb),
            ("w:left", &ml),
            ("w:header", &mh),
            ("w:footer", &mf),
            ("w:gutter", &mg),
        ],
    );

    let _ = write_end(w, "w:sectPr");
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
            write_table(w, tbl, collector);
        }
        Block::HorizontalRule => {
            write_horizontal_rule(w);
        }
        Block::CodeBlock(_, code) => {
            write_code_block(w, code, collector);
        }
        Block::BlockQuote(blocks) => {
            write_blocks(w, blocks, collector, 0);
        }
        Block::LineBlock(lines) => {
            write_line_block(w, lines, collector);
        }
        Block::Div(_, blocks) => {
            write_blocks(w, blocks, collector, 0);
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
        // Out of scope: NotesBlock, RawBlock.
        Block::NotesBlock(_) | Block::RawBlock(_, _) => {}
        // Catch-all for future variants.
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
            emit_char_props(w, cp);
            let _ = write_end(w, "w:rPr");
        }
        let _ = write_end(w, "w:pPr");
    }

    write_inlines(w, &sp.inlines, &RunProps::default(), collector);
    let _ = write_end(w, "w:p");
}

/// Emits the children of `w:pPr` from a [`ParaProps`] (no wrapper element).
fn write_para_props_inline<W: std::io::Write>(
    w: &mut Writer<W>,
    pp: &loki_doc_model::style::props::para_props::ParaProps,
) {
    use loki_doc_model::style::props::para_props::ParagraphAlignment;

    if let Some(align) = pp.alignment {
        let jc = match align {
            ParagraphAlignment::Left => "left",
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
                TabAlignment::Left => "left",
                TabAlignment::Center => "center",
                TabAlignment::Right => "right",
                TabAlignment::Decimal => "decimal",
                TabAlignment::Clear => "clear",
                _ => "left",
            };
            let pos = pts_to_twips(ts.position.value()).to_string();
            let leader = match ts.leader {
                TabLeader::None => "none",
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

        let before = pp.space_before.map(|v| match v {
            Spacing::Exact(pt) => pts_to_twips(pt.value()),
            Spacing::Percent(_) => 0,
            _ => 0,
        });
        let before_s;
        if let Some(b) = before {
            before_s = b.to_string();
            attrs.push(("w:before", &before_s));
        }

        let after = pp.space_after.map(|v| match v {
            Spacing::Exact(pt) => pts_to_twips(pt.value()),
            Spacing::Percent(_) => 0,
            _ => 0,
        });
        let after_s;
        if let Some(a) = after {
            after_s = a.to_string();
            attrs.push(("w:after", &after_s));
        }

        let line_s;
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
                _ => {
                    line_s = String::new();
                }
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

// ── Table ────────────────────────────────────────────────────────────────────

fn write_table<W: std::io::Write>(w: &mut Writer<W>, tbl: &Table, collector: &mut ExportCollector) {
    let _ = write_start(w, "w:tbl", &[]);

    // Table properties: auto width.
    let _ = write_start(w, "w:tblPr", &[]);
    let _ = write_empty(w, "w:tblW", &[("w:w", "0"), ("w:type", "auto")]);
    let _ = write_end(w, "w:tblPr");

    // Grid columns.
    let col_count = tbl.col_specs.len();
    let mut row_span_tracker = vec![0u32; col_count];
    let _ = write_start(w, "w:tblGrid", &[]);
    for col in &tbl.col_specs {
        use loki_doc_model::content::table::col::ColWidth;
        let w_twips = match col.width {
            ColWidth::Fixed(pt) => pts_to_twips(pt.value()).to_string(),
            _ => "1440".to_string(),
        };
        let _ = write_empty(w, "w:gridCol", &[("w:w", &w_twips)]);
    }
    let _ = write_end(w, "w:tblGrid");

    // Header rows.
    for row in &tbl.head.rows {
        write_table_row(w, row, true, &mut row_span_tracker, collector);
    }
    // Body rows.
    for body in &tbl.bodies {
        for row in &body.head_rows {
            write_table_row(w, row, true, &mut row_span_tracker, collector);
        }
        for row in &body.body_rows {
            write_table_row(w, row, false, &mut row_span_tracker, collector);
        }
    }
    // Foot rows.
    for row in &tbl.foot.rows {
        write_table_row(w, row, false, &mut row_span_tracker, collector);
    }

    let _ = write_end(w, "w:tbl");
}

fn write_table_row<W: std::io::Write>(
    w: &mut Writer<W>,
    row: &loki_doc_model::content::table::row::Row,
    is_header: bool,
    row_span_tracker: &mut [u32],
    collector: &mut ExportCollector,
) {
    let _ = write_start(w, "w:tr", &[]);
    if is_header {
        let _ = write_start(w, "w:trPr", &[]);
        let _ = write_empty(w, "w:tblHeader", &[]);
        let _ = write_end(w, "w:trPr");
    }

    let mut col_idx = 0;
    let mut cell_it = row.cells.iter();

    while col_idx < row_span_tracker.len() {
        if row_span_tracker[col_idx] > 0 {
            // This column is covered by a merge from above.
            let _ = write_start(w, "w:tc", &[]);
            let _ = write_start(w, "w:tcPr", &[]);
            let _ = write_empty(w, "w:vMerge", &[]);
            let _ = write_end(w, "w:tcPr");
            let _ = write_start(w, "w:p", &[]);
            let _ = write_end(w, "w:p");
            let _ = write_end(w, "w:tc");

            row_span_tracker[col_idx] -= 1;
            col_idx += 1;
        } else if let Some(cell) = cell_it.next() {
            write_table_cell(w, cell, collector);

            if cell.row_span > 1 {
                for i in 0..cell.col_span as usize {
                    if col_idx + i < row_span_tracker.len() {
                        row_span_tracker[col_idx + i] = cell.row_span - 1;
                    }
                }
            }
            col_idx += cell.col_span as usize;
        } else {
            // Should not happen in a valid model.
            break;
        }
    }

    let _ = write_end(w, "w:tr");
}

fn write_table_cell<W: std::io::Write>(
    w: &mut Writer<W>,
    cell: &Cell,
    collector: &mut ExportCollector,
) {
    let _ = write_start(w, "w:tc", &[]);

    // Cell properties.
    let _ = write_start(w, "w:tcPr", &[]);
    if cell.col_span > 1 {
        let span_s = cell.col_span.to_string();
        let _ = write_empty(w, "w:gridSpan", &wval(&span_s));
    }
    if cell.row_span > 1 {
        let _ = write_empty(w, "w:vMerge", &wval("restart"));
    }
    let props = &cell.props;
    // Padding (margins).
    let has_padding = props.padding_top.is_some()
        || props.padding_bottom.is_some()
        || props.padding_left.is_some()
        || props.padding_right.is_some();
    if has_padding {
        let _ = write_start(w, "w:tcMar", &[]);
        if let Some(pt) = props.padding_top {
            let v = pts_to_twips(pt.value()).to_string();
            let _ = write_empty(w, "w:top", &[("w:w", &v), ("w:type", "dxa")]);
        }
        if let Some(pt) = props.padding_bottom {
            let v = pts_to_twips(pt.value()).to_string();
            let _ = write_empty(w, "w:bottom", &[("w:w", &v), ("w:type", "dxa")]);
        }
        if let Some(pt) = props.padding_left {
            let v = pts_to_twips(pt.value()).to_string();
            let _ = write_empty(w, "w:left", &[("w:w", &v), ("w:type", "dxa")]);
        }
        if let Some(pt) = props.padding_right {
            let v = pts_to_twips(pt.value()).to_string();
            let _ = write_empty(w, "w:right", &[("w:w", &v), ("w:type", "dxa")]);
        }
        let _ = write_end(w, "w:tcMar");
    }
    // Vertical alignment.
    if let Some(va) = props.vertical_align {
        use loki_doc_model::content::table::row::CellVerticalAlign;
        let v = match va {
            CellVerticalAlign::Middle => "center",
            CellVerticalAlign::Bottom => "bottom",
            _ => "top",
        };
        let _ = write_empty(w, "w:vAlign", &wval(v));
    }
    // Background color (shading).
    if let Some(color) = &props.background_color {
        let hex = color_to_hex(color);
        let _ = write_empty(
            w,
            "w:shd",
            &[("w:val", "clear"), ("w:color", "auto"), ("w:fill", &hex)],
        );
    }
    let _ = write_end(w, "w:tcPr");

    // Cell content — must have at least one paragraph.
    if cell.blocks.is_empty() {
        let _ = write_start(w, "w:p", &[]);
        let _ = write_end(w, "w:p");
    } else {
        write_blocks(w, &cell.blocks, collector, 0);
    }

    let _ = write_end(w, "w:tc");
}

// ── Inline dispatch ──────────────────────────────────────────────────────────

/// Accumulated run formatting inherited from inline wrappers.
#[derive(Default, Clone)]
struct RunProps {
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

fn write_inlines<W: std::io::Write>(
    w: &mut Writer<W>,
    inlines: &[Inline],
    props: &RunProps,
    collector: &mut ExportCollector,
) {
    for inline in inlines {
        write_inline(w, inline, props, collector);
    }
}

fn write_inline<W: std::io::Write>(
    w: &mut Writer<W>,
    inline: &Inline,
    props: &RunProps,
    collector: &mut ExportCollector,
) {
    match inline {
        Inline::Str(s) => write_text_run(w, s, props),
        Inline::Space | Inline::SoftBreak => write_text_run(w, " ", props),
        Inline::LineBreak => {
            let _ = write_start(w, "w:r", &[]);
            let _ = write_empty(w, "w:br", &[]);
            let _ = write_end(w, "w:r");
        }
        Inline::Strong(inner) => {
            let np = RunProps {
                bold: true,
                ..props.clone()
            };
            write_inlines(w, inner, &np, collector);
        }
        Inline::Emph(inner) => {
            let np = RunProps {
                italic: true,
                ..props.clone()
            };
            write_inlines(w, inner, &np, collector);
        }
        Inline::Underline(inner) => {
            let np = RunProps {
                underline: true,
                ..props.clone()
            };
            write_inlines(w, inner, &np, collector);
        }
        Inline::Strikeout(inner) => {
            let np = RunProps {
                strikethrough: true,
                ..props.clone()
            };
            write_inlines(w, inner, &np, collector);
        }
        Inline::Superscript(inner) => {
            let np = RunProps {
                superscript: true,
                ..props.clone()
            };
            write_inlines(w, inner, &np, collector);
        }
        Inline::Subscript(inner) => {
            let np = RunProps {
                subscript: true,
                ..props.clone()
            };
            write_inlines(w, inner, &np, collector);
        }
        Inline::SmallCaps(inner) => {
            let np = RunProps {
                small_caps: true,
                ..props.clone()
            };
            write_inlines(w, inner, &np, collector);
        }
        Inline::Quoted(kind, inner) => {
            use loki_doc_model::content::inline::QuoteType;
            let (open, close) = match kind {
                QuoteType::SingleQuote => ("\u{2018}", "\u{2019}"),
                QuoteType::DoubleQuote => ("\u{201C}", "\u{201D}"),
            };
            write_text_run(w, open, props);
            write_inlines(w, inner, props, collector);
            write_text_run(w, close, props);
        }
        Inline::Cite(_, inner) => {
            write_inlines(w, inner, props, collector);
        }
        Inline::Code(_, s) => {
            let np = RunProps {
                code: true,
                ..props.clone()
            };
            write_text_run(w, s, &np);
        }
        Inline::Span(_, inner) => {
            write_inlines(w, inner, props, collector);
        }
        Inline::Link(_, inner, target) => {
            let r_id = collector.add_hyperlink(&target.url);
            let _ = write_start(w, "w:hyperlink", &[("r:id", &r_id)]);
            write_inlines(w, inner, props, collector);
            let _ = write_end(w, "w:hyperlink");
        }
        Inline::StyledRun(run) => {
            write_styled_run(w, run, props, collector);
        }
        Inline::Bookmark(kind, name) => {
            write_bookmark(w, kind, name);
        }
        Inline::Math(_, s) => {
            write_text_run(w, s, props);
        }
        Inline::Image(_, inlines, target) => {
            if let Some(r_id) = collector.add_image(&target.url) {
                // Default: 1 inch = 914400 EMU.
                let alt = inlines_to_string(inlines);
                let _ = write_inline_drawing(w, &r_id, 914400, 914400, &alt);
            } else {
                write_text_run(w, "[Image]", props);
            }
        }
        Inline::Note(kind, blocks) => {
            let note_id = match kind {
                NoteKind::Footnote => collector.add_footnote(blocks.clone()),
                NoteKind::Endnote => collector.add_endnote(blocks.clone()),
                _ => 0,
            };

            let _ = write_start(w, "w:r", &[]);
            let _ = write_start(w, "w:rPr", &[]);
            let _ = write_empty(w, "w:vertAlign", &wval("superscript"));
            let style = match kind {
                NoteKind::Footnote => "FootnoteReference",
                NoteKind::Endnote => "EndnoteReference",
                _ => "DefaultParagraphFont",
            };
            let _ = write_empty(w, "w:rStyle", &wval(style));
            let _ = write_end(w, "w:rPr");

            let elem = match kind {
                NoteKind::Footnote => "w:footnoteReference",
                NoteKind::Endnote => "w:endnoteReference",
                _ => "w:footnoteReference",
            };
            let _ = write_empty(w, elem, &[("w:id", &note_id.to_string())]);
            let _ = write_end(w, "w:r");
        }
        // Out of scope: Field, Comment, RawInline.
        Inline::Field(_) | Inline::Comment(_) | Inline::RawInline(_, _) => {}
        // Catch-all for future variants.
        _ => {}
    }
}

fn write_styled_run<W: std::io::Write>(
    w: &mut Writer<W>,
    run: &StyledRun,
    parent: &RunProps,
    collector: &mut ExportCollector,
) {
    let np = RunProps {
        bold: parent.bold,
        italic: parent.italic,
        underline: parent.underline,
        strikethrough: parent.strikethrough,
        superscript: parent.superscript,
        subscript: parent.subscript,
        small_caps: parent.small_caps,
        code: parent.code,
        char_style: run.style_id.as_ref().map(|s| s.0.clone()),
        direct: run.direct_props.as_deref().cloned(),
    };
    write_inlines(w, &run.content, &np, collector);
}

/// Writes a single `<w:r>` element with text content.
fn write_text_run<W: std::io::Write>(w: &mut Writer<W>, text: &str, props: &RunProps) {
    if text.is_empty() {
        return;
    }
    let _ = write_start(w, "w:r", &[]);

    // Emit w:rPr if any formatting is active.
    let has_rpr = props.bold
        || props.italic
        || props.underline
        || props.strikethrough
        || props.superscript
        || props.subscript
        || props.small_caps
        || props.code
        || props.char_style.is_some()
        || props.direct.is_some();

    if has_rpr {
        let _ = write_start(w, "w:rPr", &[]);
        if let Some(ref sid) = props.char_style {
            let _ = write_empty(w, "w:rStyle", &wval(sid));
        }
        if props.code {
            let _ = write_empty(
                w,
                "w:rFonts",
                &[("w:ascii", "Courier New"), ("w:hAnsi", "Courier New")],
            );
        }
        if props.bold {
            let _ = write_empty(w, "w:b", &[]);
        }
        if props.italic {
            let _ = write_empty(w, "w:i", &[]);
        }
        if props.small_caps {
            let _ = write_empty(w, "w:smallCaps", &[]);
        }
        if props.underline {
            let _ = write_empty(w, "w:u", &wval("single"));
        }
        if props.strikethrough {
            let _ = write_empty(w, "w:strike", &[]);
        }
        if props.superscript {
            let _ = write_empty(w, "w:vertAlign", &wval("superscript"));
        } else if props.subscript {
            let _ = write_empty(w, "w:vertAlign", &wval("subscript"));
        }
        if let Some(ref cp) = props.direct {
            emit_char_props(w, cp);
        }
        let _ = write_end(w, "w:rPr");
    }

    // Text node — always use xml:space="preserve" to keep leading/trailing spaces.
    let _ = write_empty_checked(w, text);
    let _ = write_end(w, "w:r");
}

/// Writes `<w:t xml:space="preserve">text</w:t>`.
fn write_empty_checked<W: std::io::Write>(w: &mut Writer<W>, text: &str) -> quick_xml::Result<()> {
    use quick_xml::events::{BytesStart, BytesText, Event};
    let mut start = BytesStart::new("w:t");
    start.push_attribute(("xml:space", "preserve"));
    w.write_event(Event::Start(start))?;
    w.write_event(Event::Text(BytesText::new(text)))?;
    w.write_event(Event::End(quick_xml::events::BytesEnd::new("w:t")))
}

static BOOKMARK_COUNTER: std::sync::atomic::AtomicU32 = std::sync::atomic::AtomicU32::new(0);

fn write_bookmark<W: std::io::Write>(
    w: &mut Writer<W>,
    kind: &loki_doc_model::content::inline::BookmarkKind,
    name: &str,
) {
    use loki_doc_model::content::inline::BookmarkKind;
    let id = BOOKMARK_COUNTER.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
    let id_s = id.to_string();
    match kind {
        BookmarkKind::Start => {
            let _ = write_empty(w, "w:bookmarkStart", &[("w:id", &id_s), ("w:name", name)]);
        }
        BookmarkKind::End => {
            let _ = write_empty(w, "w:bookmarkEnd", &[("w:id", &id_s)]);
        }
    }
}

fn write_inline_drawing<W: std::io::Write>(
    w: &mut Writer<W>,
    r_id: &str,
    cx: u64,
    cy: u64,
    alt: &str,
) -> quick_xml::Result<()> {
    let _ = write_start(w, "w:r", &[]);
    let _ = write_start(w, "w:drawing", &[]);
    let _ = write_start(
        w,
        "wp:inline",
        &[
            ("distT", "0"),
            ("distB", "0"),
            ("distL", "0"),
            ("distR", "0"),
        ],
    );

    let cx_s = cx.to_string();
    let cy_s = cy.to_string();
    let _ = write_empty(w, "wp:extent", &[("cx", &cx_s), ("cy", &cy_s)]);
    let _ = write_empty(
        w,
        "wp:docPr",
        &[("id", "1"), ("name", "Image"), ("descr", alt)],
    );

    let _ = write_start(w, "a:graphic", &[("xmlns:a", NS_A)]);
    let _ = write_start(
        w,
        "a:graphicData",
        &[(
            "uri",
            "http://schemas.openxmlformats.org/drawingml/2006/picture",
        )],
    );

    let _ = write_start(w, "pic:pic", &[("xmlns:pic", NS_PIC)]);

    // pic:nvPicPr
    let _ = write_start(w, "pic:nvPicPr", &[]);
    let _ = write_empty(w, "pic:cNvPr", &[("id", "0"), ("name", "")]);
    let _ = write_empty(w, "pic:cNvPicPr", &[]);
    let _ = write_end(w, "pic:nvPicPr");

    // pic:blipFill
    let _ = write_start(w, "pic:blipFill", &[]);
    let _ = write_empty(w, "a:blip", &[("r:embed", r_id), ("xmlns:r", NS_R)]);
    let _ = write_start(w, "a:stretch", &[]);
    let _ = write_empty(w, "a:fillRect", &[]);
    let _ = write_end(w, "a:stretch");
    let _ = write_end(w, "pic:blipFill");

    // pic:spPr
    let _ = write_start(w, "pic:spPr", &[]);
    let _ = write_start(w, "a:xfrm", &[]);
    let _ = write_empty(w, "a:off", &[("x", "0"), ("y", "0")]);
    let _ = write_empty(w, "a:ext", &[("cx", &cx_s), ("cy", &cy_s)]);
    let _ = write_end(w, "a:xfrm");
    let _ = write_start(w, "a:prstGeom", &[("prst", "rect")]);
    let _ = write_empty(w, "a:avLst", &[]);
    let _ = write_end(w, "a:prstGeom");
    let _ = write_end(w, "pic:spPr");

    let _ = write_end(w, "pic:pic");
    let _ = write_end(w, "a:graphicData");
    let _ = write_end(w, "a:graphic");
    let _ = write_end(w, "wp:inline");
    let _ = write_end(w, "w:drawing");
    let _ = write_end(w, "w:r");

    Ok(())
}

fn inlines_to_string(inlines: &[Inline]) -> String {
    let mut s = String::new();
    for inline in inlines {
        match inline {
            Inline::Str(t) => s.push_str(t),
            Inline::Space | Inline::SoftBreak => s.push(' '),
            Inline::LineBreak => s.push('\n'),
            Inline::Strong(i)
            | Inline::Emph(i)
            | Inline::Underline(i)
            | Inline::Strikeout(i)
            | Inline::Superscript(i)
            | Inline::Subscript(i)
            | Inline::SmallCaps(i)
            | Inline::Quoted(_, i)
            | Inline::Cite(_, i)
            | Inline::Span(_, i)
            | Inline::Link(_, i, _)
            | Inline::Image(_, i, _) => s.push_str(&inlines_to_string(i)),
            Inline::Code(_, t) | Inline::Math(_, t) => s.push_str(t),
            Inline::StyledRun(run) => s.push_str(&inlines_to_string(&run.content)),
            _ => {}
        }
    }
    s
}
