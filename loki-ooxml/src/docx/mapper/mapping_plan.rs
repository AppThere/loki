// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 AppThere Loki contributors

//! Mapping plan — audit tables for the OOXML → doc-model translation layer.
//!
//! This module contains no executable code; it documents the intermediate
//! model inventory, the doc-model inventory, and the mapping decisions made
//! for the DOCX import pipeline.
//!
//! # Intermediate model inventory (loki-ooxml)
//!
//! | Intermediate type      | Location                    | Fields / gaps            |
//! |------------------------|-----------------------------|--------------------------|
//! | DocxDocument           | model/document.rs           | body: DocxBody           |
//! | DocxBody               | model/document.rs           | children, final_sect_pr  |
//! | DocxBodyChild          | model/document.rs           | Paragraph | Table | Sdt  |
//! | DocxParagraph          | model/paragraph.rs          | ppr, children            |
//! | DocxPPr                | model/paragraph.rs          | style_id, jc, ind,       |
//! |                        |                             | spacing, num_pr,         |
//! |                        |                             | outline_lvl, keep_lines, |
//! |                        |                             | keep_next, page_break,   |
//! |                        |                             | widow_control, bidi,     |
//! |                        |                             | sect_pr                  |
//! | DocxSectPr             | model/paragraph.rs          | pg_sz, pg_mar,           |
//! |                        |                             | header_refs, footer_refs |
//! | DocxRun                | model/paragraph.rs          | rpr, children            |
//! | DocxRPr                | model/paragraph.rs          | bold, italic, underline, |
//! |                        |                             | strike, dstrike, color,  |
//! |                        |                             | highlight, sz, sz_cs,    |
//! |                        |                             | fonts, kern, spacing,    |
//! |                        |                             | scale, lang, vert_align, |
//! |                        |                             | small_caps, all_caps,    |
//! |                        |                             | shadow, style_id         |
//! | DocxRunChild           | model/paragraph.rs          | Text | Break | FldChar  |
//! |                        |                             | InstrText | FootnoteRef  |
//! |                        |                             | EndnoteRef | Drawing | Tab|
//! | DocxParaChild          | model/paragraph.rs          | Run | Hyperlink |         |
//! |                        |                             | BookmarkStart/End |       |
//! |                        |                             | TrackDel | TrackIns      |
//! | DocxHyperlink          | model/paragraph.rs          | rel_id, anchor, runs     |
//! | DocxDrawing            | model/paragraph.rs          | rel_id, cx, cy, descr,   |
//! |                        |                             | name, is_anchor          |
//! | DocxStyles             | model/styles.rs             | default_ppr, default_rpr,|
//! |                        |                             | styles                   |
//! | DocxStyle              | model/styles.rs             | type, id, is_default,    |
//! |                        |                             | is_custom, name,         |
//! |                        |                             | based_on, next, link,    |
//! |                        |                             | ppr, rpr                 |
//! | DocxStyleType          | model/styles.rs             | Paragraph | Character |  |
//! |                        |                             | Table | Numbering        |
//! | DocxTableModel         | model/styles.rs             | tbl_pr, col_widths, rows |
//! | DocxTableRow           | model/styles.rs             | tr_pr, cells             |
//! | DocxTableCell          | model/styles.rs             | tc_pr, paragraphs        |
//! | DocxTrPr               | model/styles.rs             | is_header                |
//! | DocxTcPr               | model/styles.rs             | grid_span, v_merge       |
//! | DocxNumbering          | model/numbering.rs          | abstract_nums, nums      |
//! | DocxAbstractNum        | model/numbering.rs          | abstract_num_id, levels  |
//! | DocxNum                | model/numbering.rs          | num_id, abstract_num_id, |
//! |                        |                             | level_overrides          |
//! | DocxLevel              | model/numbering.rs          | ilvl, start, num_fmt,    |
//! |                        |                             | lvl_text, lvl_jc, ppr,   |
//! |                        |                             | rpr                      |
//! | DocxLvlOverride        | model/numbering.rs          | ilvl, start_override,    |
//! |                        |                             | level                    |
//! | DocxNotes              | model/footnotes.rs          | notes                    |
//! | DocxNote               | model/footnotes.rs          | id, note_type, paragraphs|
//! | DocxNoteType           | model/footnotes.rs          | Normal | Separator |      |
//! |                        |                             | ContinuationSeparator    |
//! | DocxSettings           | model/settings.rs           | default_tab_stop,        |
//! |                        |                             | even_and_odd_headers,    |
//! |                        |                             | title_pg                 |
//!
//! Parsed but not yet modelled: DocxSettings fields are parsed but unused downstream.
//! Not parsed at all: headers/footer XML parts (header_refs/footer_refs collected but
//! content not read), comments, revision history, theme, custom XML, forms.
//!
//! # doc-model inventory (loki-doc-model)
//!
//! | doc-model type         | Location                    | Constructor / builder    |
//! |------------------------|-----------------------------|--------------------------|
//! | Document               | src/document.rs             | struct literal / new()   |
//! | DocumentMeta           | src/meta/core.rs            | struct literal / default |
//! | StyleCatalog           | src/style/catalog.rs        | new() / default          |
//! | ParagraphStyle         | src/style/para_style.rs     | struct literal           |
//! | CharacterStyle         | src/style/char_style.rs     | struct literal           |
//! | TableStyle             | src/style/table_style.rs    | struct literal           |
//! | ListStyle              | src/style/list_style.rs     | struct literal           |
//! | ListLevel              | src/style/list_style.rs     | struct literal           |
//! | ListLevelKind          | src/style/list_style.rs     | enum variant             |
//! | ParaProps              | src/style/props/para_props  | struct literal / default |
//! | CharProps              | src/style/props/char_props  | struct literal / default |
//! | Section                | src/layout/section.rs       | new() / with_layout...() |
//! | PageLayout             | src/layout/page.rs          | struct literal / default |
//! | PageSize               | src/layout/page.rs          | a4() / letter() / custom |
//! | PageMargins            | src/layout/page.rs          | struct literal / default |
//! | Block                  | src/content/block.rs        | enum variants            |
//! | StyledParagraph        | src/content/block.rs        | struct literal           |
//! | Table                  | src/content/table/core.rs   | struct literal           |
//! | Row / Cell             | src/content/table/row.rs    | struct literal           |
//! | Inline                 | src/content/inline.rs       | enum variants            |
//! | StyledRun              | src/content/inline.rs       | struct literal           |
//! | Field                  | src/content/field/mod.rs    | new(kind)                |
//! | FieldKind              | src/content/field/types.rs  | enum variants            |
//!
//! # Mapping plan
//!
//! ## Straightforward mappings
//!
//! | OOXML source             | doc-model target               | Notes            |
//! |--------------------------|--------------------------------|------------------|
//! | DocxDocument body        | Document.sections              | split on sectPr  |
//! | DocxSectPr (pg_sz/mar)   | Section.layout (PageLayout)    | twips → pt       |
//! | DocxParagraph            | Block::StyledPara              | or Block::Heading|
//! | DocxPPr                  | ParaProps + style_id           | twips → pt       |
//! | DocxRun                  | Inline::Str or StyledRun       | wrap if styled   |
//! | DocxRPr                  | CharProps                      | half-pt → pt     |
//! | DocxHyperlink            | Inline::Link                   | rel_id resolved  |
//! | DocxDrawing              | Inline::Image                  | data URI opt.    |
//! | DocxStyles               | StyleCatalog                   | 3 style types    |
//! | DocxStyle (Paragraph)    | ParagraphStyle                 | parent ref kept  |
//! | DocxStyle (Character)    | CharacterStyle                 | parent ref kept  |
//! | DocxStyle (Table)        | TableStyle                     | minimal props    |
//! | DocxNumbering            | ListStyle per num_id           | 3-level resolved |
//! | DocxNote (Normal)        | Inline::Note blocks            | pre-processed    |
//! | CoreProperties           | DocumentMeta                   | OPC meta         |
//! | DocxTableModel           | Block::Table                   | tblGrid → ColSpec|
//! | DocxFldChar/InstrText    | Inline::Field                  | state machine    |
//! | BookmarkStart/End        | Inline::Bookmark               | id/name preserved|
//!
//! ## Stubbed / defaulted
//!
//! | OOXML source             | Status                         | Reason           |
//! |--------------------------|--------------------------------|------------------|
//! | DocxSectPr header/footer | Implemented — Session 7        | gap #5 P1        |
//! | DocxStyle (Numbering)    | Skipped silently               | Handled via num. |
//! | DocxBodyChild::Sdt       | Skipped                        | No model equiv.  |
//! | DocxTcPr.v_merge         | Stubbed row_span = 1           | Track NYI v0.1.0 |
//! | DocxSettings             | even_and_odd_headers wired     | Session 7        |
//! | DocxNote (Separator)     | Filtered out                   | Not semantic     |
//! | TrackDel content         | Dropped                        | Deleted content  |
//!
//! # Session 7 audit (2026-04-20) — gap #5: headers and footers
//!
//! | Item                          | Pre-S7 status | Action needed              |
//! |-------------------------------|---------------|----------------------------|
//! | DocxSectPr header/footer refs | Parsed        | Part loading missing       |
//! | titlePg in DocxSectPr         | Missing       | Add field + parse          |
//! | evenAndOddHeaders             | Parsed        | Thread through pipeline    |
//! | Rel ID → XML part path        | Missing       | Load via REL_HEADER/FOOTER |
//! | Header/footer XML parser      | Missing       | New reader/header_footer.rs|
//! | loki-doc-model HF type        | Complete      | HeaderFooter, PageLayout.* |
//! | LayoutPage.header_items type  | Vec<PI>       | Add header_height f32      |
//! | paint_single_page HF render   | Complete      | Use page-local coords      |
//!
//! Design decisions:
//! - title_page: per-section (ECMA-376 §17.6.17), added to DocxSectPr.
//! - even_and_odd_headers: document-level (§17.15.1.25), from DocxSettings.
//! - PageLayout already has all 6 HF fields; no new doc-model types needed.
//! - Header/footer XML reuses parse_paragraph from document.rs (same structure).
//! - All header/footer parts pre-loaded in import.rs by REL_HEADER / REL_FOOTER.
//! - layout_blocks_reflow in flow.rs calls flow_section in Reflow mode to lay out
//!   HF content; synthetic section has no HF fields → no infinite recursion.
//! - HF items translated to page-local coords before storing in LayoutPage.
//! - LayoutPage gains header_height / footer_height for downstream use.
//!
//! # Impedance mismatches (resolved as follows)
//!
//! - Style inheritance: OOXML uses named `basedOn` chains. doc-model stores
//!   `parent: Option<StyleId>` references. Resolution is deferred to the layout
//!   engine; the mapper preserves the reference, no flattening done here.
//! - Outline level: OOXML is 0-indexed (0 = Heading 1); doc-model is 1-indexed.
//!   Conversion: level + 1. Controlled by DocxImportOptions::emit_heading_blocks.
//! - Numbering indirection: w:numId → w:num → w:abstractNum → levels. Fully
//!   resolved in numbering.rs before writing to StyleCatalog::list_styles.
//! - Measurements: OOXML twips (1/20 pt), half-points (1/2 pt), EMUs (1/12700 pt).
//!   All converted to Points at the mapper boundary.
//! - Section splits: w:sectPr may appear inside w:pPr (mid-document break) or as
//!   the final child of w:body. Both cases produce a new Section.
//! - Complex fields: w:fldChar begin/instrText/separate/result/end spans multiple
//!   runs. Assembled via a state machine in inline.rs.
