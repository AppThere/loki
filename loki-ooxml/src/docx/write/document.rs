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
use loki_doc_model::content::inline::{Inline, StyledRun};
use loki_doc_model::content::table::core::Table;
use loki_doc_model::content::table::row::Cell;
use loki_doc_model::layout::page::PageLayout;
use loki_doc_model::layout::section::Section;
use loki_doc_model::style::catalog::StyleCatalog;
use loki_doc_model::style::props::char_props::CharProps;

use crate::docx::write::numbering::NumberingState;
use crate::docx::write::styles::emit_char_props;
use crate::docx::write::xml::{
    pts_to_twips, write_decl, write_empty, write_end, write_start, wval, NS_R, NS_W,
};

/// Serializes all sections to `word/document.xml` bytes.
/// `num_state` is populated with any lists encountered.
pub(super) fn write_document_xml(
    sections: &[Section],
    _catalog: &StyleCatalog,
    num_state: &mut NumberingState,
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
        ],
    );
    let _ = write_start(&mut w, "w:body", &[]);

    for (idx, section) in sections.iter().enumerate() {
        let is_last = idx + 1 == sections.len();
        write_blocks(&mut w, &section.blocks, num_state, 0);

        // Emit w:sectPr — for the last section it is a direct child of w:body;
        // for earlier sections it goes inside a final empty paragraph.
        let layout = &section.layout;
        if is_last {
            write_sect_pr(&mut w, layout);
        } else {
            let _ = write_start(&mut w, "w:p", &[]);
            let _ = write_start(&mut w, "w:pPr", &[]);
            write_sect_pr(&mut w, layout);
            let _ = write_end(&mut w, "w:pPr");
            let _ = write_end(&mut w, "w:p");
        }
    }

    if sections.is_empty() {
        // Always emit at least one empty paragraph and a sectPr.
        let _ = write_start(&mut w, "w:p", &[]);
        let _ = write_end(&mut w, "w:p");
        write_sect_pr(&mut w, &PageLayout::default());
    }

    let _ = write_end(&mut w, "w:body");
    let _ = write_end(&mut w, "w:document");
    drop(w);
    out
}

// ── Section properties ───────────────────────────────────────────────────────

fn write_sect_pr<W: std::io::Write>(w: &mut Writer<W>, layout: &PageLayout) {
    let _ = write_start(w, "w:sectPr", &[]);

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

// ── Block dispatch ───────────────────────────────────────────────────────────

/// Recursively writes a slice of blocks.  `list_level` tracks nesting depth
/// for nested lists (currently always 0 since we only support ilvl=0).
fn write_blocks<W: std::io::Write>(
    w: &mut Writer<W>,
    blocks: &[Block],
    num_state: &mut NumberingState,
    list_level: u8,
) {
    for block in blocks {
        write_block(w, block, num_state, list_level);
    }
}

fn write_block<W: std::io::Write>(
    w: &mut Writer<W>,
    block: &Block,
    num_state: &mut NumberingState,
    _list_level: u8,
) {
    match block {
        Block::Para(inlines) | Block::Plain(inlines) => {
            write_para(w, None, None, inlines);
        }
        Block::StyledPara(sp) => {
            write_styled_para(w, sp);
        }
        Block::Heading(level, _, inlines) => {
            let style_id = format!("Heading{level}");
            write_para(w, Some(&style_id), None, inlines);
        }
        Block::BulletList(items) => {
            let num_id = num_state.register_bullet();
            for item_blocks in items {
                write_list_item(w, item_blocks, num_id, 0, num_state);
            }
        }
        Block::OrderedList(attrs, items) => {
            let num_id = num_state.register_ordered(attrs);
            for item_blocks in items {
                write_list_item(w, item_blocks, num_id, 0, num_state);
            }
        }
        Block::Table(tbl) => {
            write_table(w, tbl, num_state);
        }
        Block::HorizontalRule => {
            write_horizontal_rule(w);
        }
        Block::CodeBlock(_, code) => {
            write_code_block(w, code);
        }
        Block::BlockQuote(blocks) => {
            write_blocks(w, blocks, num_state, 0);
        }
        Block::LineBlock(lines) => {
            write_line_block(w, lines);
        }
        Block::Div(_, blocks) => {
            write_blocks(w, blocks, num_state, 0);
        }
        Block::DefinitionList(items) => {
            for (term, defs) in items {
                write_para(w, None, None, term);
                for def_blocks in defs {
                    write_blocks(w, def_blocks, num_state, 0);
                }
            }
        }
        Block::TableOfContents(toc) => {
            write_blocks(w, &toc.body, num_state, 0);
        }
        Block::Index(idx) => {
            write_blocks(w, &idx.body, num_state, 0);
        }
        // Out of scope for Tier 3: Figure, NotesBlock, RawBlock.
        Block::Figure(_, _, _) | Block::NotesBlock(_) | Block::RawBlock(_, _) => {}
        // Catch-all for future variants.
        _ => {}
    }
}

// ── Paragraph helpers ────────────────────────────────────────────────────────

/// Writes `<w:p>` with optional `w:pStyle` and optional `w:numPr`.
fn write_para<W: std::io::Write>(
    w: &mut Writer<W>,
    style_id: Option<&str>,
    num_pr: Option<(u32, u8)>,  // (numId, ilvl)
    inlines: &[Inline],
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

    write_inlines(w, inlines, &RunProps::default());
    let _ = write_end(w, "w:p");
}

fn write_styled_para<W: std::io::Write>(w: &mut Writer<W>, sp: &StyledParagraph) {
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

    write_inlines(w, &sp.inlines, &RunProps::default());
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

    let has_ind = pp.indent_start.is_some() || pp.indent_end.is_some() || pp.indent_hanging.is_some();
    if has_ind {
        let left = pp.indent_start.map_or(0, |v| pts_to_twips(v.value())).to_string();
        let right = pp.indent_end.map_or(0, |v| pts_to_twips(v.value())).to_string();
        let hanging = pp.indent_hanging.map_or(0, |v| pts_to_twips(v.value())).to_string();
        let mut attrs: Vec<(&str, &str)> = Vec::new();
        if pp.indent_start.is_some() { attrs.push(("w:left", &left)); }
        if pp.indent_end.is_some() { attrs.push(("w:right", &right)); }
        if pp.indent_hanging.is_some() { attrs.push(("w:hanging", &hanging)); }
        if !attrs.is_empty() {
            let _ = write_empty(w, "w:ind", &attrs);
        }
    }

    if pp.space_before.is_some() || pp.space_after.is_some() {
        use loki_doc_model::style::props::para_props::Spacing;
        let before = pp.space_before.map_or(0, |v| match v {
            Spacing::Exact(pt) => pts_to_twips(pt.value()),
            Spacing::Percent(_) | _ => 0,
        }).to_string();
        let after = pp.space_after.map_or(0, |v| match v {
            Spacing::Exact(pt) => pts_to_twips(pt.value()),
            Spacing::Percent(_) | _ => 0,
        }).to_string();
        let _ = write_empty(w, "w:spacing", &[("w:before", &before), ("w:after", &after)]);
    }
}

fn write_code_block<W: std::io::Write>(w: &mut Writer<W>, code: &str) {
    let _ = write_start(w, "w:p", &[]);
    let _ = write_start(w, "w:pPr", &[]);
    let _ = write_empty(w, "w:pStyle", &wval("Code"));
    let _ = write_end(w, "w:pPr");
    write_text_run(w, code, &RunProps { code: true, ..Default::default() });
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

fn write_line_block<W: std::io::Write>(w: &mut Writer<W>, lines: &[Vec<Inline>]) {
    for (idx, line) in lines.iter().enumerate() {
        let _ = write_start(w, "w:p", &[]);
        write_inlines(w, line, &RunProps::default());
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
    num_state: &mut NumberingState,
) {
    let mut first = true;
    for block in blocks {
        if first {
            first = false;
            match block {
                Block::Para(inlines) | Block::Plain(inlines) => {
                    write_para(w, None, Some((num_id, ilvl)), inlines);
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
                    write_inlines(w, &sp.inlines, &RunProps::default());
                    let _ = write_end(w, "w:p");
                }
                // For non-para first block, emit an empty list para + recurse.
                other => {
                    write_para(w, None, Some((num_id, ilvl)), &[]);
                    write_block(w, other, num_state, ilvl);
                }
            }
        } else {
            write_block(w, block, num_state, ilvl);
        }
    }
    if blocks.is_empty() {
        write_para(w, None, Some((num_id, ilvl)), &[]);
    }
}

// ── Table ────────────────────────────────────────────────────────────────────

fn write_table<W: std::io::Write>(
    w: &mut Writer<W>,
    tbl: &Table,
    num_state: &mut NumberingState,
) {
    let _ = write_start(w, "w:tbl", &[]);

    // Table properties: auto width.
    let _ = write_start(w, "w:tblPr", &[]);
    let _ = write_empty(w, "w:tblW", &[("w:w", "0"), ("w:type", "auto")]);
    let _ = write_end(w, "w:tblPr");

    // Grid columns.
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
        write_table_row(w, row, true, num_state);
    }
    // Body rows.
    for body in &tbl.bodies {
        for row in &body.head_rows {
            write_table_row(w, row, true, num_state);
        }
        for row in &body.body_rows {
            write_table_row(w, row, false, num_state);
        }
    }
    // Foot rows.
    for row in &tbl.foot.rows {
        write_table_row(w, row, false, num_state);
    }

    let _ = write_end(w, "w:tbl");
}

fn write_table_row<W: std::io::Write>(
    w: &mut Writer<W>,
    row: &loki_doc_model::content::table::row::Row,
    is_header: bool,
    num_state: &mut NumberingState,
) {
    let _ = write_start(w, "w:tr", &[]);
    if is_header {
        let _ = write_start(w, "w:trPr", &[]);
        let _ = write_empty(w, "w:tblHeader", &[]);
        let _ = write_end(w, "w:trPr");
    }
    for cell in &row.cells {
        write_table_cell(w, cell, num_state);
    }
    let _ = write_end(w, "w:tr");
}

fn write_table_cell<W: std::io::Write>(
    w: &mut Writer<W>,
    cell: &Cell,
    num_state: &mut NumberingState,
) {
    let _ = write_start(w, "w:tc", &[]);

    // Cell properties.
    let _ = write_start(w, "w:tcPr", &[]);
    if cell.col_span > 1 {
        let span_s = cell.col_span.to_string();
        let _ = write_empty(w, "w:gridSpan", &wval(&span_s));
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
    let _ = write_end(w, "w:tcPr");

    // Cell content — must have at least one paragraph.
    if cell.blocks.is_empty() {
        let _ = write_start(w, "w:p", &[]);
        let _ = write_end(w, "w:p");
    } else {
        write_blocks(w, &cell.blocks, num_state, 0);
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
) {
    for inline in inlines {
        write_inline(w, inline, props);
    }
}

fn write_inline<W: std::io::Write>(w: &mut Writer<W>, inline: &Inline, props: &RunProps) {
    match inline {
        Inline::Str(s) => write_text_run(w, s, props),
        Inline::Space | Inline::SoftBreak => write_text_run(w, " ", props),
        Inline::LineBreak => {
            let _ = write_start(w, "w:r", &[]);
            let _ = write_empty(w, "w:br", &[]);
            let _ = write_end(w, "w:r");
        }
        Inline::Strong(inner) => {
            let np = RunProps { bold: true, ..props.clone() };
            write_inlines(w, inner, &np);
        }
        Inline::Emph(inner) => {
            let np = RunProps { italic: true, ..props.clone() };
            write_inlines(w, inner, &np);
        }
        Inline::Underline(inner) => {
            let np = RunProps { underline: true, ..props.clone() };
            write_inlines(w, inner, &np);
        }
        Inline::Strikeout(inner) => {
            let np = RunProps { strikethrough: true, ..props.clone() };
            write_inlines(w, inner, &np);
        }
        Inline::Superscript(inner) => {
            let np = RunProps { superscript: true, ..props.clone() };
            write_inlines(w, inner, &np);
        }
        Inline::Subscript(inner) => {
            let np = RunProps { subscript: true, ..props.clone() };
            write_inlines(w, inner, &np);
        }
        Inline::SmallCaps(inner) => {
            let np = RunProps { small_caps: true, ..props.clone() };
            write_inlines(w, inner, &np);
        }
        Inline::Quoted(kind, inner) => {
            use loki_doc_model::content::inline::QuoteType;
            let (open, close) = match kind {
                QuoteType::SingleQuote => ("\u{2018}", "\u{2019}"),
                QuoteType::DoubleQuote => ("\u{201C}", "\u{201D}"),
            };
            write_text_run(w, open, props);
            write_inlines(w, inner, props);
            write_text_run(w, close, props);
        }
        Inline::Cite(_, inner) => {
            write_inlines(w, inner, props);
        }
        Inline::Code(_, s) => {
            let np = RunProps { code: true, ..props.clone() };
            write_text_run(w, s, &np);
        }
        Inline::Span(_, inner) => {
            write_inlines(w, inner, props);
        }
        Inline::Link(_, inner, _target) => {
            // Hyperlinks require relationship IDs — emit link text only.
            write_inlines(w, inner, props);
        }
        Inline::StyledRun(run) => {
            write_styled_run(w, run, props);
        }
        Inline::Bookmark(kind, name) => {
            write_bookmark(w, kind, name);
        }
        Inline::Math(_, s) => {
            write_text_run(w, s, props);
        }
        // Out of scope: Image, Note, Field, Comment, RawInline.
        Inline::Image(_, _, _)
        | Inline::Note(_, _)
        | Inline::Field(_)
        | Inline::Comment(_)
        | Inline::RawInline(_, _) => {}
        // Catch-all for future variants.
        _ => {}
    }
}

fn write_styled_run<W: std::io::Write>(w: &mut Writer<W>, run: &StyledRun, parent: &RunProps) {
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
    write_inlines(w, &run.content, &np);
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
                &[
                    ("w:ascii", "Courier New"),
                    ("w:hAnsi", "Courier New"),
                ],
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
