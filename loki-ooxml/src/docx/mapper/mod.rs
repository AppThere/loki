// Copyright 2026 AppThere Loki contributors
// SPDX-License-Identifier: Apache-2.0

//! Translates intermediate OOXML model types → [`loki_doc_model`] types.
//!
//! This is the second step of the two-step DOCX import pipeline:
//!
//! 1. **XML → intermediate model** (`reader` layer)
//! 2. **Intermediate model → [`loki_doc_model`]** (this layer)
//!
//! Entry point: [`map_document`].

// ══════════════════════════════════════════════════════════════════════════════
// Audit and mapping plan (Step 1 of implementation task)
// ══════════════════════════════════════════════════════════════════════════════
//
// ── 1a. loki-ooxml intermediate model inventory ───────────────────────────────
//
// | Intermediate type      | Location                    | Fields / gaps            |
// |------------------------|-----------------------------|--------------------------|
// | DocxDocument           | model/document.rs           | body: DocxBody           |
// | DocxBody               | model/document.rs           | children, final_sect_pr  |
// | DocxBodyChild          | model/document.rs           | Paragraph | Table | Sdt  |
// | DocxParagraph          | model/paragraph.rs          | ppr, children            |
// | DocxPPr                | model/paragraph.rs          | style_id, jc, ind,       |
// |                        |                             | spacing, num_pr,         |
// |                        |                             | outline_lvl, keep_lines, |
// |                        |                             | keep_next, page_break,   |
// |                        |                             | widow_control, bidi,     |
// |                        |                             | sect_pr                  |
// | DocxSectPr             | model/paragraph.rs          | pg_sz, pg_mar,           |
// |                        |                             | header_refs, footer_refs |
// | DocxRun                | model/paragraph.rs          | rpr, children            |
// | DocxRPr                | model/paragraph.rs          | bold, italic, underline, |
// |                        |                             | strike, dstrike, color,  |
// |                        |                             | highlight, sz, sz_cs,    |
// |                        |                             | fonts, kern, spacing,    |
// |                        |                             | scale, lang, vert_align, |
// |                        |                             | small_caps, all_caps,    |
// |                        |                             | shadow, style_id         |
// | DocxRunChild           | model/paragraph.rs          | Text | Break | FldChar  |
// |                        |                             | InstrText | FootnoteRef  |
// |                        |                             | EndnoteRef | Drawing | Tab|
// | DocxParaChild          | model/paragraph.rs          | Run | Hyperlink |         |
// |                        |                             | BookmarkStart/End |       |
// |                        |                             | TrackDel | TrackIns      |
// | DocxHyperlink          | model/paragraph.rs          | rel_id, anchor, runs     |
// | DocxDrawing            | model/paragraph.rs          | rel_id, cx, cy, descr,   |
// |                        |                             | name, is_anchor          |
// | DocxStyles             | model/styles.rs             | default_ppr, default_rpr,|
// |                        |                             | styles                   |
// | DocxStyle              | model/styles.rs             | type, id, is_default,    |
// |                        |                             | is_custom, name,         |
// |                        |                             | based_on, next, link,    |
// |                        |                             | ppr, rpr                 |
// | DocxStyleType          | model/styles.rs             | Paragraph | Character |  |
// |                        |                             | Table | Numbering        |
// | DocxTableModel         | model/styles.rs             | tbl_pr, col_widths, rows |
// | DocxTableRow           | model/styles.rs             | tr_pr, cells             |
// | DocxTableCell          | model/styles.rs             | tc_pr, paragraphs        |
// | DocxTrPr               | model/styles.rs             | is_header                |
// | DocxTcPr               | model/styles.rs             | grid_span, v_merge       |
// | DocxNumbering          | model/numbering.rs          | abstract_nums, nums      |
// | DocxAbstractNum        | model/numbering.rs          | abstract_num_id, levels  |
// | DocxNum                | model/numbering.rs          | num_id, abstract_num_id, |
// |                        |                             | level_overrides          |
// | DocxLevel              | model/numbering.rs          | ilvl, start, num_fmt,    |
// |                        |                             | lvl_text, lvl_jc, ppr,   |
// |                        |                             | rpr                      |
// | DocxLvlOverride        | model/numbering.rs          | ilvl, start_override,    |
// |                        |                             | level                    |
// | DocxNotes              | model/footnotes.rs          | notes                    |
// | DocxNote               | model/footnotes.rs          | id, note_type, paragraphs|
// | DocxNoteType           | model/footnotes.rs          | Normal | Separator |      |
// |                        |                             | ContinuationSeparator    |
// | DocxSettings           | model/settings.rs           | default_tab_stop,        |
// |                        |                             | even_and_odd_headers,    |
// |                        |                             | title_pg                 |
//
// Parsed but not yet modelled: DocxSettings fields are parsed but unused downstream.
// Not parsed at all: headers/footer XML parts (header_refs/footer_refs collected but
// content not read), comments, revision history, theme, custom XML, forms.
//
// ── 1b. loki-doc-model inventory ─────────────────────────────────────────────
//
// | doc-model type         | Location                    | Constructor / builder    |
// |------------------------|-----------------------------|--------------------------|
// | Document               | src/document.rs             | struct literal / new()   |
// | DocumentMeta           | src/meta/core.rs            | struct literal / default |
// | StyleCatalog           | src/style/catalog.rs        | new() / default          |
// | ParagraphStyle         | src/style/para_style.rs     | struct literal           |
// | CharacterStyle         | src/style/char_style.rs     | struct literal           |
// | TableStyle             | src/style/table_style.rs    | struct literal           |
// | ListStyle              | src/style/list_style.rs     | struct literal           |
// | ListLevel              | src/style/list_style.rs     | struct literal           |
// | ListLevelKind          | src/style/list_style.rs     | enum variant             |
// | ParaProps              | src/style/props/para_props  | struct literal / default |
// | CharProps              | src/style/props/char_props  | struct literal / default |
// | Section                | src/layout/section.rs       | new() / with_layout...() |
// | PageLayout             | src/layout/page.rs          | struct literal / default |
// | PageSize               | src/layout/page.rs          | a4() / letter() / custom |
// | PageMargins            | src/layout/page.rs          | struct literal / default |
// | Block                  | src/content/block.rs        | enum variants            |
// | StyledParagraph        | src/content/block.rs        | struct literal           |
// | Table                  | src/content/table/core.rs   | struct literal           |
// | Row / Cell             | src/content/table/row.rs    | struct literal           |
// | Inline                 | src/content/inline.rs       | enum variants            |
// | StyledRun              | src/content/inline.rs       | struct literal           |
// | Field                  | src/content/field/mod.rs    | new(kind)                |
// | FieldKind              | src/content/field/types.rs  | enum variants            |
//
// ── 1c. Mapping plan ─────────────────────────────────────────────────────────
//
// STRAIGHTFORWARD MAPPINGS
// ┌──────────────────────────┬────────────────────────────────┬──────────────────┐
// │ OOXML source             │ doc-model target               │ Notes            │
// ├──────────────────────────┼────────────────────────────────┼──────────────────┤
// │ DocxDocument body        │ Document.sections              │ split on sectPr  │
// │ DocxSectPr (pg_sz/mar)   │ Section.layout (PageLayout)   │ twips → pt       │
// │ DocxParagraph            │ Block::StyledPara              │ or Block::Heading│
// │ DocxPPr                  │ ParaProps + style_id          │ twips → pt       │
// │ DocxRun                  │ Inline::Str or StyledRun       │ wrap if styled   │
// │ DocxRPr                  │ CharProps                      │ half-pt → pt     │
// │ DocxHyperlink            │ Inline::Link                   │ rel_id resolved  │
// │ DocxDrawing              │ Inline::Image                  │ data URI opt.    │
// │ DocxStyles               │ StyleCatalog                   │ 3 style types    │
// │ DocxStyle (Paragraph)    │ ParagraphStyle                 │ parent ref kept  │
// │ DocxStyle (Character)    │ CharacterStyle                 │ parent ref kept  │
// │ DocxStyle (Table)        │ TableStyle                     │ minimal props    │
// │ DocxNumbering            │ ListStyle per num_id           │ 3-level resolved │
// │ DocxNote (Normal)        │ Inline::Note blocks            │ pre-processed    │
// │ CoreProperties           │ DocumentMeta                   │ OPC meta         │
// │ DocxTableModel           │ Block::Table                   │ tblGrid → ColSpec│
// │ DocxFldChar/InstrText    │ Inline::Field                  │ state machine    │
// │ BookmarkStart/End        │ Inline::Bookmark               │ id/name preserved│
// └──────────────────────────┴────────────────────────────────┴──────────────────┘
//
// STUBBED / DEFAULTED (no doc-model equivalent or not yet implemented)
// ┌──────────────────────────┬────────────────────────────────┬──────────────────┐
// │ OOXML source             │ Status                         │ Reason           │
// ├──────────────────────────┼────────────────────────────────┼──────────────────┤
// │ DocxSectPr header/footer │ Implemented — Session 7        │ gap #5 P1        │
// │ DocxStyle (Numbering)    │ Skipped silently               │ Handled via num. │
// │ DocxBodyChild::Sdt       │ Skipped                        │ No model equiv.  │
// │ DocxTcPr.v_merge         │ Stubbed row_span = 1           │ Track NYI v0.1.0 │
// │ DocxSettings             │ even_and_odd_headers wired     │ Session 7        │
// │ DocxNote (Separator)     │ Filtered out                   │ Not semantic     │
// │ TrackDel content         │ Dropped                        │ Deleted content  │
// └──────────────────────────┴────────────────────────────────┴──────────────────┘
//
// ── Session 7 audit (2026-04-20) — gap #5: headers and footers ───────────────
//
// Readiness table before any Session 7 code was written:
//
// | Item                          | Pre-S7 status | Action needed              |
// |-------------------------------|---------------|----------------------------|
// | DocxSectPr header/footer refs | Parsed        | Part loading missing       |
// | titlePg in DocxSectPr         | Missing       | Add field + parse          |
// | evenAndOddHeaders             | Parsed        | Thread through pipeline    |
// | Rel ID → XML part path        | Missing       | Load via REL_HEADER/FOOTER |
// | Header/footer XML parser      | Missing       | New reader/header_footer.rs|
// | loki-doc-model HF type        | Complete      | HeaderFooter, PageLayout.* |
// | LayoutPage.header_items type  | Vec<PI>       | Add header_height f32      |
// | paint_single_page HF render   | Complete      | Use page-local coords      |
//
// Design decisions:
// • title_page: per-section (ECMA-376 §17.6.17), added to DocxSectPr.
// • even_and_odd_headers: document-level (§17.15.1.25), from DocxSettings.
// • PageLayout already has all 6 HF fields; no new doc-model types needed.
// • Header/footer XML reuses parse_paragraph from document.rs (same structure).
// • All header/footer parts pre-loaded in import.rs by REL_HEADER / REL_FOOTER.
// • layout_blocks_reflow in flow.rs calls flow_section in Reflow mode to lay out
//   HF content; synthetic section has no HF fields → no infinite recursion.
// • HF items translated to page-local coords before storing in LayoutPage.
// • LayoutPage gains header_height / footer_height for downstream use.
//
// IMPEDANCE MISMATCHES (resolved as follows)
// • Style inheritance: OOXML uses named `basedOn` chains. doc-model stores
//   `parent: Option<StyleId>` references. Resolution is deferred to the layout
//   engine; the mapper preserves the reference, no flattening done here (see
//   StyleCatalog::resolve_para / resolve_char which walk the chain at read time).
// • Outline level: OOXML is 0-indexed (0 = Heading 1); doc-model is 1-indexed.
//   Conversion: level + 1. Controlled by DocxImportOptions::emit_heading_blocks.
// • Numbering indirection: w:numId → w:num → w:abstractNum → levels. Fully
//   resolved in numbering.rs before writing to StyleCatalog::list_styles.
// • Measurements: OOXML twips (1/20 pt), half-points (1/2 pt), EMUs (1/12700 pt).
//   All converted to Points at the mapper boundary.
// • Section splits: w:sectPr may appear inside w:pPr (mid-document break) or as
//   the final child of w:body. Both cases produce a new Section.
// • Complex fields: w:fldChar begin/instrText/separate/result/end spans multiple
//   runs. Assembled via a state machine in inline.rs.

// ══════════════════════════════════════════════════════════════════════════════
// Module declarations
// ══════════════════════════════════════════════════════════════════════════════

pub mod error;
pub use error::MapperError;

pub(crate) mod document;
pub(crate) mod images;
pub(crate) mod inline;
pub(crate) mod numbering;
pub(crate) mod paragraph;
pub(crate) mod props;
pub(crate) mod styles;
pub(crate) mod table;

// DocxSettings.even_and_odd_headers is now wired through map_document (Session 7).
// DocxSettings.default_tab_stop and title_pg remain unused pending further work.

// ══════════════════════════════════════════════════════════════════════════════
// Public entry point
// ══════════════════════════════════════════════════════════════════════════════

use loki_doc_model::document::Document;
use loki_opc::Package;

use crate::docx::import::{parse_and_map_package, DocxImportOptions};

/// Maps an OOXML OPC [`Package`] to a format-neutral [`Document`].
///
/// This is the primary public mapper entry point. It handles both the XML
/// parsing step (reading DOCX parts from the package) and the model-mapping
/// step (translating the intermediate model to [`loki_doc_model`]).
///
/// Non-fatal import warnings (unresolved relationships, unsupported features,
/// etc.) are discarded. For access to warnings, use
/// [`crate::docx::import::DocxImporter::run`] instead.
///
/// # Errors
///
/// Returns [`MapperError::Pipeline`] if the package is missing a required
/// part (e.g. the `officeDocument` relationship), if a mandatory XML part
/// cannot be parsed, or if an OPC-level error occurs. Optional or
/// enrichment-only parts (styles, numbering, footnotes) map to defaults
/// rather than erroring when absent.
///
/// Returns [`MapperError::MissingRequiredElement`] if a required OOXML
/// element is absent in the intermediate model.
///
/// Returns [`MapperError::InvalidValue`] if an element carries a value that
/// is structurally invalid.
///
/// # Examples
///
/// ```no_run
/// use std::fs::File;
/// use loki_ooxml::docx::mapper::map_document;
/// use loki_ooxml::docx::import::DocxImportOptions;
/// use loki_opc::Package;
///
/// let file = File::open("document.docx")?;
/// let package = Package::open(file)?;
/// let doc = map_document(&package, &DocxImportOptions::default())?;
/// assert!(!doc.sections.is_empty());
/// # Ok::<(), Box<dyn std::error::Error>>(())
/// ```
pub fn map_document(
    package: &Package,
    options: &DocxImportOptions,
) -> Result<Document, MapperError> {
    let (doc, _warnings) = parse_and_map_package(package, options)?;
    Ok(doc)
}

// ── Tests ─────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use loki_opc::{PartData, PartName};
    use loki_opc::relationships::{Relationship, TargetMode};

    const REL_OFFICE_DOCUMENT: &str =
        "http://schemas.openxmlformats.org/officeDocument/2006/relationships/officeDocument";
    const MEDIA_TYPE_DOCUMENT: &str =
        "application/vnd.openxmlformats-officedocument.wordprocessingml.document.main+xml";

    /// Builds a minimal in-memory DOCX OPC package programmatically.
    ///
    /// Contains only the parts needed by `map_document`: the package-level
    /// `officeDocument` relationship and a valid `word/document.xml`.
    fn make_package(doc_xml: &[u8]) -> Package {
        let mut pkg = Package::new();
        let part_name = PartName::new("/word/document.xml").unwrap();
        pkg.set_part(
            part_name,
            PartData::new(doc_xml.to_vec(), MEDIA_TYPE_DOCUMENT),
        );
        pkg.relationships_mut()
            .add(Relationship {
                id: "rId1".into(),
                rel_type: REL_OFFICE_DOCUMENT.into(),
                target: "/word/document.xml".into(),
                target_mode: TargetMode::Internal,
            })
            .unwrap();
        pkg
    }

    // ── Round-trip test ───────────────────────────────────────────────────────

    #[test]
    fn round_trip_minimal_document() {
        let package = make_package(
            br#"<?xml version="1.0" encoding="UTF-8"?>
<w:document xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main">
  <w:body>
    <w:p><w:r><w:t>Hello, world!</w:t></w:r></w:p>
    <w:sectPr><w:pgSz w:w="11906" w:h="16838"/></w:sectPr>
  </w:body>
</w:document>"#,
        );
        let doc = map_document(&package, &DocxImportOptions::default())
            .expect("map_document must succeed for a minimal package");

        assert!(!doc.sections.is_empty(), "at least one section expected");
        let blocks = &doc.sections[0].blocks;
        assert!(!blocks.is_empty(), "paragraph should be present");

        use loki_doc_model::content::block::Block;
        assert!(
            matches!(blocks[0], Block::StyledPara(_)),
            "first block should be StyledPara, got {:?}",
            blocks[0]
        );
    }

    // ── Optional absent: no styles part → empty catalog, no error ────────────

    #[test]
    fn missing_styles_part_uses_defaults() {
        let package = make_package(
            br#"<?xml version="1.0" encoding="UTF-8"?>
<w:document xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main">
  <w:body/>
</w:document>"#,
        );
        let doc = map_document(&package, &DocxImportOptions::default())
            .expect("missing styles part should not error");
        // No styles were loaded; catalog is empty.
        assert!(doc.styles.paragraph_styles.is_empty());
    }

    // ── MapperError variants display correctly ────────────────────────────────

    #[test]
    fn missing_required_element_message() {
        let e = MapperError::MissingRequiredElement { element: "w:body" };
        assert!(e.to_string().contains("w:body"));
    }

    #[test]
    fn invalid_value_message() {
        let e = MapperError::InvalidValue {
            element: "w:pgSz",
            detail: "width must be positive".into(),
        };
        let s = e.to_string();
        assert!(s.contains("w:pgSz"));
        assert!(s.contains("width must be positive"));
    }

    // ── A4 defaults when no sectPr present ────────────────────────────────────

    #[test]
    fn no_sect_pr_yields_a4_layout() {
        let package = make_package(
            br#"<?xml version="1.0" encoding="UTF-8"?>
<w:document xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main">
  <w:body/>
</w:document>"#,
        );
        let doc = map_document(&package, &DocxImportOptions::default()).unwrap();
        assert_eq!(doc.sections.len(), 1);
        let sz = &doc.sections[0].layout.page_size;
        // A4: 595.28 × 841.89 pt — allow ±0.1 pt tolerance.
        assert!((sz.width.value() - 595.28).abs() < 0.1, "A4 width expected, got {}", sz.width.value());
        assert!((sz.height.value() - 841.89).abs() < 0.1, "A4 height expected, got {}", sz.height.value());
    }

    // ── Pipeline error for missing officeDocument relationship ────────────────

    #[test]
    fn missing_office_document_rel_yields_pipeline_error() {
        // An empty package has no officeDocument relationship.
        let pkg = Package::new();
        let result = map_document(&pkg, &DocxImportOptions::default());
        assert!(result.is_err());
        assert!(
            matches!(result.unwrap_err(), MapperError::Pipeline(_)),
            "expected Pipeline error variant"
        );
    }
}
