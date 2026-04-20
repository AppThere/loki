// Copyright 2026 AppThere Loki contributors
// SPDX-License-Identifier: Apache-2.0

//! Document-level mapper: converts the ODF intermediate representation into
//! the format-neutral [`loki_doc_model::Document`].
//!
//! # Entry point
//!
//! [`map_document`] is the top-level conversion function, called by
//! [`crate::odt::import::OdtImporter::run`] after all XML parts have been
//! parsed. It coordinates:
//!
//! 1. Stylesheet → [`StyleCatalog`] via [`super::styles::map_stylesheet`]
//! 2. List styles → inserted into the same [`StyleCatalog`]
//! 3. Body content → [`Block`]s via recursive descent helpers
//! 4. Active master page → [`PageLayout`]
//! 5. Metadata → [`DocumentMeta`]

use std::collections::HashMap;

use loki_doc_model::content::attr::{ExtensionBag, NodeAttr};
use loki_doc_model::content::block::{
    Block, Caption, ListAttributes, ListDelimiter, ListNumberStyle,
    StyledParagraph, TableOfContentsBlock,
};
use loki_doc_model::content::field::types::{CrossRefFormat, Field, FieldKind};
use loki_doc_model::content::inline::{
    BookmarkKind, Inline, LinkTarget, NoteKind, StyledRun,
};
use loki_doc_model::content::table::col::{ColAlignment, ColSpec};
use loki_doc_model::content::table::core::{
    Table, TableBody, TableCaption, TableFoot, TableHead,
};
use loki_doc_model::content::table::row::{Cell, Row};
use loki_doc_model::document::Document;
use loki_doc_model::layout::page::{
    PageLayout, PageMargins, PageOrientation, PageSize,
};
use loki_doc_model::layout::section::Section;
use loki_doc_model::meta::core::DocumentMeta;
use loki_doc_model::style::catalog::{StyleCatalog, StyleId};
use loki_doc_model::style::list_style::{ListId, ListLevelKind, NumberingScheme};
use loki_primitives::units::Points;

use crate::error::OdfWarning;
use crate::odt::import::OdtImportOptions;
use crate::odt::mapper::lists::map_list_styles;
use crate::odt::mapper::styles::map_stylesheet;
use crate::odt::model::document::{
    OdfBodyChild, OdfDocument, OdfList, OdfListItem, OdfListItemChild,
    OdfMeta, OdfPageLayout, OdfSection, OdfTableOfContent,
};
use crate::odt::model::fields::OdfField;
use crate::odt::model::frames::{OdfFrame, OdfFrameKind};
use crate::odt::model::notes::{OdfNote, OdfNoteClass};
use crate::odt::model::paragraph::{
    OdfHyperlink, OdfParagraph, OdfParagraphChild, OdfSpan,
};
use crate::odt::model::styles::OdfStylesheet;
use crate::odt::model::tables::OdfTable;
use crate::xml_util::parse_length;

// ── Context ────────────────────────────────────────────────────────────────────

/// State threaded through all mapping helpers during a single
/// [`map_document`] call.
///
/// Holds read-only references to the resolved catalog, image store, and import
/// options, plus mutable collections for warnings and for floating figures that
/// were encountered inside inline content and need to be emitted as block-level
/// siblings after their host paragraph.
pub(crate) struct OdfMappingContext<'a> {
    /// The fully-built style catalog (paragraph, character, list styles).
    pub styles: &'a StyleCatalog,
    /// Images extracted from the ODF package: ZIP-entry path →
    /// (media-type, raw bytes).
    pub images: &'a HashMap<String, (String, Vec<u8>)>,
    /// Import options controlling heading emission, image embedding, etc.
    pub options: &'a OdtImportOptions,
    /// Non-fatal issues accumulated during mapping.
    pub warnings: Vec<OdfWarning>,
    /// Floating frames (images and text boxes that are not `as-char` anchored)
    /// collected while mapping inline content. The caller flushes this after
    /// each paragraph or block.
    pub pending_figures: Vec<Block>,
}

// ── Public entry point ─────────────────────────────────────────────────────────

/// Convert a fully-parsed ODF document into a format-neutral
/// [`loki_doc_model::Document`] plus a list of non-fatal [`OdfWarning`]s.
///
/// The [`crate::odt::import::OdtImporter`] calls this after reading all
/// package parts. The returned document's `source` field is left `None`;
/// the caller sets it with the correct [`OdfVersion`]-derived string.
///
/// [`OdfVersion`]: crate::version::OdfVersion
pub(crate) fn map_document(
    doc: &OdfDocument,
    stylesheet: &OdfStylesheet,
    meta: Option<&OdfMeta>,
    images: &HashMap<String, (String, Vec<u8>)>,
    options: &OdtImportOptions,
) -> (Document, Vec<OdfWarning>) {
    // ── 1. Map stylesheet + list styles ──────────────────────────────────────
    let mut catalog = map_stylesheet(stylesheet);
    map_list_styles(&stylesheet.list_styles, &mut catalog, doc.version);

    // ── 2. Resolve active page layout ─────────────────────────────────────────
    let page_layout = resolve_page_layout(stylesheet);

    // ── 3. Map body (scoped so the &catalog borrow ends before we move it) ────
    let (blocks, warnings) = {
        let mut ctx = OdfMappingContext {
            styles: &catalog,
            images,
            options,
            warnings: Vec::new(),
            pending_figures: Vec::new(),
        };
        let blocks = map_body_children(&doc.body_children, &mut ctx);
        (blocks, ctx.warnings)
    };

    // ── 4. Assemble section ───────────────────────────────────────────────────
    let section = Section::with_layout_and_blocks(page_layout, blocks);

    // ── 5. Map metadata ───────────────────────────────────────────────────────
    let doc_meta = meta.map(map_meta).unwrap_or_default();

    // ── 6. Build document (caller sets source) ────────────────────────────────
    let document = Document {
        meta: doc_meta,
        styles: catalog,
        sections: vec![section],
        source: None,
    };

    (document, warnings)
}

// ── Body ───────────────────────────────────────────────────────────────────────

/// Convert a slice of [`OdfBodyChild`]s into [`Block`]s, flushing any
/// pending floating figures after each block.
fn map_body_children(
    children: &[OdfBodyChild],
    ctx: &mut OdfMappingContext<'_>,
) -> Vec<Block> {
    let mut blocks = Vec::new();
    for child in children {
        if let Some(block) = map_body_child(child, ctx) {
            blocks.push(block);
            let figures = std::mem::take(&mut ctx.pending_figures);
            blocks.extend(figures);
        }
    }
    blocks
}

fn map_body_child(
    child: &OdfBodyChild,
    ctx: &mut OdfMappingContext<'_>,
) -> Option<Block> {
    match child {
        OdfBodyChild::Paragraph(para) | OdfBodyChild::Heading(para) => {
            Some(map_paragraph(para, ctx))
        }
        OdfBodyChild::List(list) => Some(map_list(list, ctx)),
        OdfBodyChild::Table(table) => Some(map_table(table, ctx)),
        OdfBodyChild::TableOfContent(toc) => Some(map_toc(toc, ctx)),
        OdfBodyChild::Section(section) => Some(map_section(section, ctx)),
        OdfBodyChild::Other => None,
    }
}

// ── Paragraphs ─────────────────────────────────────────────────────────────────

/// Convert an [`OdfParagraph`] to either [`Block::Heading`] (when
/// `is_heading` and `emit_heading_blocks` are both true) or
/// [`Block::StyledPara`].
fn map_paragraph(para: &OdfParagraph, ctx: &mut OdfMappingContext<'_>) -> Block {
    let inlines = map_inline_children(&para.children, ctx);

    if para.is_heading && ctx.options.emit_heading_blocks {
        let level = para.outline_level.unwrap_or(1).clamp(1, 6);
        // Store the ODF style name in NodeAttr so the layout engine can look up
        // heading style properties from the catalog. Without this, the flow engine
        // falls back to hardcoded "Heading1"/"Heading2" IDs which don't match ODF
        // names like "Heading_20_1" (LibreOffice-encoded space).
        let mut attr = NodeAttr::default();
        if let Some(ref sn) = para.style_name {
            attr.kv.push(("style".to_string(), sn.clone()));
        }
        Block::Heading(level, attr, inlines)
    } else {
        let style_id = para.style_name.as_deref().map(StyleId::new);
        Block::StyledPara(StyledParagraph {
            style_id,
            direct_para_props: None,
            direct_char_props: None,
            inlines,
            attr: NodeAttr::default(),
        })
    }
}

// ── Inlines ────────────────────────────────────────────────────────────────────

fn map_inline_children(
    children: &[OdfParagraphChild],
    ctx: &mut OdfMappingContext<'_>,
) -> Vec<Inline> {
    children.iter().filter_map(|c| map_inline(c, ctx)).collect()
}

fn map_inline(
    child: &OdfParagraphChild,
    ctx: &mut OdfMappingContext<'_>,
) -> Option<Inline> {
    match child {
        OdfParagraphChild::Text(s) => {
            if s.is_empty() {
                None
            } else {
                Some(Inline::Str(s.clone()))
            }
        }
        OdfParagraphChild::Span(span) => Some(map_span(span, ctx)),
        OdfParagraphChild::Hyperlink(link) => Some(map_hyperlink(link, ctx)),
        OdfParagraphChild::Note(note) => Some(map_note(note, ctx)),
        OdfParagraphChild::Bookmark { name, .. } => {
            Some(Inline::Bookmark(BookmarkKind::Start, name.clone()))
        }
        OdfParagraphChild::BookmarkEnd { name } => {
            Some(Inline::Bookmark(BookmarkKind::End, name.clone()))
        }
        OdfParagraphChild::Field(field) => Some(Inline::Field(map_field(field))),
        OdfParagraphChild::Frame(frame) => map_frame(frame, ctx),
        OdfParagraphChild::SoftReturn => None,
        OdfParagraphChild::Tab => Some(Inline::Str("\t".into())),
        OdfParagraphChild::Space { count } => {
            Some(Inline::Str(" ".repeat(*count as usize)))
        }
        OdfParagraphChild::LineBreak => Some(Inline::LineBreak),
        OdfParagraphChild::Other => None,
    }
}

fn map_span(span: &OdfSpan, ctx: &mut OdfMappingContext<'_>) -> Inline {
    let style_id = span.style_name.as_deref().map(StyleId::new);
    let content = map_inline_children(&span.children, ctx);
    Inline::StyledRun(StyledRun {
        style_id,
        direct_props: None,
        content,
        attr: NodeAttr::default(),
    })
}

fn map_hyperlink(link: &OdfHyperlink, ctx: &mut OdfMappingContext<'_>) -> Inline {
    let href = link.href.clone().unwrap_or_default();
    let content = map_inline_children(&link.children, ctx);
    Inline::Link(NodeAttr::default(), content, LinkTarget::new(href))
}

fn map_note(note: &OdfNote, ctx: &mut OdfMappingContext<'_>) -> Inline {
    let kind = match note.note_class {
        OdfNoteClass::Footnote => NoteKind::Footnote,
        OdfNoteClass::Endnote => NoteKind::Endnote,
    };
    let body: Vec<Block> = note
        .body
        .iter()
        .flat_map(|p| {
            let block = map_paragraph(p, ctx);
            let figs = std::mem::take(&mut ctx.pending_figures);
            std::iter::once(block).chain(figs)
        })
        .collect();
    Inline::Note(kind, body)
}

// ── Fields ─────────────────────────────────────────────────────────────────────

fn map_field(odf: &OdfField) -> Field {
    let kind = match odf {
        OdfField::PageNumber { .. } => FieldKind::PageNumber,
        OdfField::PageCount => FieldKind::PageCount,
        OdfField::Date { data_style, .. } => {
            FieldKind::Date { format: data_style.clone() }
        }
        OdfField::Time { data_style, .. } => {
            FieldKind::Time { format: data_style.clone() }
        }
        OdfField::Title => FieldKind::Title,
        OdfField::Subject => FieldKind::Subject,
        OdfField::AuthorName => FieldKind::Author,
        OdfField::FileName { .. } => FieldKind::FileName,
        OdfField::WordCount => FieldKind::WordCount,
        OdfField::CrossReference { ref_name, display } => {
            let format = match display.as_deref() {
                Some("number") => CrossRefFormat::Number,
                Some("page") => CrossRefFormat::Page,
                Some("caption") => CrossRefFormat::Caption,
                _ => CrossRefFormat::HeadingText,
            };
            FieldKind::CrossReference { target: ref_name.clone(), format }
        }
        OdfField::ChapterName { display_levels } => FieldKind::Raw {
            instruction: format!("chapter display-levels={display_levels}"),
        },
        OdfField::Unknown { local_name, .. } => FieldKind::Raw {
            instruction: local_name.clone(),
        },
    };
    Field { kind, current_value: None, extensions: ExtensionBag::default() }
}

// ── Frames ─────────────────────────────────────────────────────────────────────

/// Map an ODF drawing frame to an inline element.
///
/// For `as-char`-anchored frames, the mapped element is returned directly.
/// For floating frames, a [`Block::Figure`] or [`Block::Div`] is pushed to
/// [`OdfMappingContext::pending_figures`] and `None` is returned.
fn map_frame(frame: &OdfFrame, ctx: &mut OdfMappingContext<'_>) -> Option<Inline> {
    let is_as_char = frame.anchor_type.as_deref() == Some("as-char");

    match &frame.kind {
        OdfFrameKind::Image { href, media_type, title, desc } => {
            if !ctx.options.embed_images {
                return None;
            }
            let (stored_mt, bytes) = match ctx.images.get(href.as_str()) {
                Some(pair) => pair,
                None => {
                    ctx.warnings
                        .push(OdfWarning::MissingImage { href: href.clone() });
                    return None;
                }
            };
            use base64::Engine as _;
            let b64 =
                base64::engine::general_purpose::STANDARD.encode(bytes);
            let mt = media_type.as_deref().unwrap_or(stored_mt.as_str());
            let data_uri = format!("data:{mt};base64,{b64}");
            let alt: Vec<Inline> = desc
                .as_deref()
                .or(title.as_deref())
                .map(|s| vec![Inline::Str(s.into())])
                .unwrap_or_default();
            let img =
                Inline::Image(NodeAttr::default(), alt, LinkTarget::new(data_uri));
            if is_as_char {
                Some(img)
            } else {
                ctx.pending_figures.push(Block::Figure(
                    NodeAttr::default(),
                    Caption::default(),
                    vec![Block::Para(vec![img])],
                ));
                None
            }
        }
        OdfFrameKind::TextBox { paragraphs } => {
            // Map the text box content as a Div pushed to pending_figures.
            let inner: Vec<Block> = paragraphs
                .iter()
                .flat_map(|p| {
                    let block = map_paragraph(p, ctx);
                    let figs = std::mem::take(&mut ctx.pending_figures);
                    std::iter::once(block).chain(figs)
                })
                .collect();
            ctx.pending_figures
                .push(Block::Div(NodeAttr::default(), inner));
            None
        }
        OdfFrameKind::Other => None,
    }
}

// ── Lists ──────────────────────────────────────────────────────────────────────

fn map_list(list: &OdfList, ctx: &mut OdfMappingContext<'_>) -> Block {
    let ordered = is_ordered_list(list.style_name.as_deref(), ctx.styles);
    let items: Vec<Vec<Block>> =
        list.items.iter().map(|item| map_list_item(item, ctx)).collect();

    if ordered {
        let attrs = build_list_attributes(list, ctx.styles);
        Block::OrderedList(attrs, items)
    } else {
        Block::BulletList(items)
    }
}

fn map_list_item(
    item: &OdfListItem,
    ctx: &mut OdfMappingContext<'_>,
) -> Vec<Block> {
    let mut blocks = Vec::new();
    for child in &item.children {
        match child {
            OdfListItemChild::Paragraph(p) | OdfListItemChild::Heading(p) => {
                let block = map_paragraph(p, ctx);
                blocks.push(block);
                let figs = std::mem::take(&mut ctx.pending_figures);
                blocks.extend(figs);
            }
            OdfListItemChild::List(nested) => {
                blocks.push(map_list(nested, ctx));
                let figs = std::mem::take(&mut ctx.pending_figures);
                blocks.extend(figs);
            }
        }
    }
    blocks
}

/// Returns `true` when the first level of the named list style is numbered.
fn is_ordered_list(style_name: Option<&str>, catalog: &StyleCatalog) -> bool {
    let name = match style_name {
        Some(n) => n,
        None => return false,
    };
    let ls = match catalog.list_styles.get(&ListId::new(name)) {
        Some(s) => s,
        None => return false,
    };
    ls.levels
        .first()
        .map(|l| matches!(l.kind, ListLevelKind::Numbered { .. }))
        .unwrap_or(false)
}

/// Build [`ListAttributes`] from the first level of the named list style.
fn build_list_attributes(
    list: &OdfList,
    catalog: &StyleCatalog,
) -> ListAttributes {
    let default = ListAttributes::default();
    let name = match list.style_name.as_deref() {
        Some(n) => n,
        None => return default,
    };
    let ls = match catalog.list_styles.get(&ListId::new(name)) {
        Some(s) => s,
        None => return default,
    };
    let first = match ls.levels.first() {
        Some(l) => l,
        None => return default,
    };
    match &first.kind {
        ListLevelKind::Numbered { scheme, start_value, format, .. } => {
            let style = match scheme {
                NumberingScheme::Decimal => ListNumberStyle::Decimal,
                NumberingScheme::LowerAlpha => ListNumberStyle::LowerAlpha,
                NumberingScheme::UpperAlpha => ListNumberStyle::UpperAlpha,
                NumberingScheme::LowerRoman => ListNumberStyle::LowerRoman,
                NumberingScheme::UpperRoman => ListNumberStyle::UpperRoman,
                _ => ListNumberStyle::Decimal,
            };
            let delimiter = if format.ends_with('.') {
                ListDelimiter::Period
            } else if format.ends_with(')') {
                ListDelimiter::OneParen
            } else {
                ListDelimiter::DefaultDelim
            };
            ListAttributes {
                start_number: *start_value as i32,
                style,
                delimiter,
            }
        }
        _ => default,
    }
}

// ── Tables ─────────────────────────────────────────────────────────────────────

fn map_table(table: &OdfTable, ctx: &mut OdfMappingContext<'_>) -> Block {
    // Expand repeated column definitions
    let col_specs: Vec<ColSpec> = table
        .col_defs
        .iter()
        .flat_map(|def| {
            let count = def.columns_repeated.max(1) as usize;
            std::iter::repeat_with(|| ColSpec::proportional(1.0)).take(count)
        })
        .collect();

    let body_rows: Vec<Row> = table
        .rows
        .iter()
        .map(|odf_row| {
            let cells: Vec<Cell> = odf_row
                .cells
                .iter()
                .map(|odf_cell| {
                    let blocks: Vec<Block> = odf_cell
                        .paragraphs
                        .iter()
                        .flat_map(|p| {
                            let block = map_paragraph(p, ctx);
                            let figs =
                                std::mem::take(&mut ctx.pending_figures);
                            std::iter::once(block).chain(figs)
                        })
                        .collect();
                    Cell {
                        attr: NodeAttr::default(),
                        alignment: ColAlignment::Default,
                        row_span: odf_cell.row_span,
                        col_span: odf_cell.col_span,
                        blocks,
                        props: Default::default(),
                    }
                })
                .collect();
            Row::new(cells)
        })
        .collect();

    Block::Table(Box::new(Table {
        attr: NodeAttr::default(),
        caption: TableCaption::default(),
        col_specs,
        head: TableHead::empty(),
        bodies: vec![TableBody::from_rows(body_rows)],
        foot: TableFoot::empty(),
    }))
}

// ── Table of contents ──────────────────────────────────────────────────────────

fn map_toc(
    toc: &OdfTableOfContent,
    ctx: &mut OdfMappingContext<'_>,
) -> Block {
    let body: Vec<Block> = toc
        .body_paragraphs
        .iter()
        .flat_map(|p| {
            let block = map_paragraph(p, ctx);
            let figs = std::mem::take(&mut ctx.pending_figures);
            std::iter::once(block).chain(figs)
        })
        .collect();
    Block::TableOfContents(TableOfContentsBlock {
        title: None,
        body,
        attr: NodeAttr::default(),
    })
}

// ── Sections ───────────────────────────────────────────────────────────────────

fn map_section(
    section: &OdfSection,
    ctx: &mut OdfMappingContext<'_>,
) -> Block {
    let blocks = map_body_children(&section.children, ctx);
    Block::Div(NodeAttr::default(), blocks)
}

// ── Page layout ────────────────────────────────────────────────────────────────

/// Find the active master page ("Standard" or the first one) and convert its
/// associated `style:page-layout` to a format-neutral [`PageLayout`].
///
/// Falls back to [`PageLayout::default`] when no master page or page layout
/// is present in the stylesheet.
fn resolve_page_layout(stylesheet: &OdfStylesheet) -> PageLayout {
    let master = stylesheet
        .master_pages
        .iter()
        .find(|m| m.name == "Standard" || m.name == "Default")
        .or_else(|| stylesheet.master_pages.first());

    let odf_layout = master.and_then(|m| {
        stylesheet
            .page_layouts
            .iter()
            .find(|pl| pl.name == m.page_layout_name)
    });

    match odf_layout {
        Some(pl) => convert_page_layout(pl),
        None => PageLayout::default(),
    }
}

fn convert_page_layout(pl: &OdfPageLayout) -> PageLayout {
    let zero = Points::new(0.0);
    let width = pl
        .page_width
        .as_deref()
        .and_then(parse_length)
        .unwrap_or_else(|| Points::new(595.28));
    let height = pl
        .page_height
        .as_deref()
        .and_then(parse_length)
        .unwrap_or_else(|| Points::new(841.89));
    let mt = pl
        .margin_top
        .as_deref()
        .and_then(parse_length)
        .unwrap_or_else(|| Points::new(72.0));
    let mb = pl
        .margin_bottom
        .as_deref()
        .and_then(parse_length)
        .unwrap_or_else(|| Points::new(72.0));
    let ml = pl
        .margin_left
        .as_deref()
        .and_then(parse_length)
        .unwrap_or_else(|| Points::new(72.0));
    let mr = pl
        .margin_right
        .as_deref()
        .and_then(parse_length)
        .unwrap_or_else(|| Points::new(72.0));

    let orientation = match pl.print_orientation.as_deref() {
        Some("landscape") => PageOrientation::Landscape,
        _ => PageOrientation::Portrait,
    };

    PageLayout {
        page_size: PageSize { width, height },
        margins: PageMargins {
            top: mt,
            bottom: mb,
            left: ml,
            right: mr,
            header: Points::new(36.0),
            footer: Points::new(36.0),
            gutter: zero,
        },
        orientation,
        ..Default::default()
    }
}

// ── Metadata ───────────────────────────────────────────────────────────────────

fn map_meta(meta: &OdfMeta) -> DocumentMeta {
    DocumentMeta {
        title: meta.title.clone(),
        description: meta.description.clone(),
        // ODF dc:creator is the person who last saved (= last_modified_by)
        last_modified_by: meta.creator.clone(),
        created: meta.created.as_deref().and_then(parse_datetime),
        modified: meta.modified.as_deref().and_then(parse_datetime),
        revision: meta.editing_cycles,
        ..Default::default()
    }
}

/// Parse an ISO-8601 / RFC-3339 datetime string into a UTC
/// [`chrono::DateTime`].
///
/// Tries RFC 3339 first (e.g. `"2024-01-15T10:30:00Z"`); falls back to
/// `"%Y-%m-%dT%H:%M:%S"` for strings without a timezone suffix.
fn parse_datetime(s: &str) -> Option<chrono::DateTime<chrono::Utc>> {
    chrono::DateTime::parse_from_rfc3339(s)
        .map(|dt| dt.with_timezone(&chrono::Utc))
        .ok()
        .or_else(|| {
            chrono::NaiveDateTime::parse_from_str(s, "%Y-%m-%dT%H:%M:%S")
                .map(|ndt| ndt.and_utc())
                .ok()
        })
}

// ── Tests ──────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use crate::odt::model::document::{OdfBodyChild, OdfDocument};
    use crate::odt::model::paragraph::{OdfParagraph, OdfParagraphChild};
    use crate::odt::model::styles::OdfStylesheet;
    use crate::version::OdfVersion;

    fn empty_stylesheet() -> OdfStylesheet {
        OdfStylesheet::default()
    }

    fn options() -> OdtImportOptions {
        OdtImportOptions::default()
    }

    fn empty_doc(children: Vec<OdfBodyChild>) -> OdfDocument {
        OdfDocument {
            version: OdfVersion::V1_2,
            version_was_absent: false,
            body_children: children,
        }
    }

    fn text_paragraph(text: &str, is_heading: bool, level: Option<u8>) -> OdfParagraph {
        OdfParagraph {
            style_name: None,
            outline_level: level,
            is_heading,
            children: vec![OdfParagraphChild::Text(text.into())],
            list_context: None,
        }
    }

    #[test]
    fn empty_document_produces_empty_section() {
        let doc = empty_doc(vec![]);
        let (result, warnings) = map_document(
            &doc,
            &empty_stylesheet(),
            None,
            &HashMap::new(),
            &options(),
        );
        assert!(warnings.is_empty());
        assert_eq!(result.sections.len(), 1);
        assert!(result.sections[0].blocks.is_empty());
    }

    #[test]
    fn heading_is_emitted_as_heading_block() {
        let para = text_paragraph("Title", true, Some(1));
        let doc = empty_doc(vec![OdfBodyChild::Heading(para)]);
        let (result, _) = map_document(
            &doc,
            &empty_stylesheet(),
            None,
            &HashMap::new(),
            &options(),
        );
        let blocks = &result.sections[0].blocks;
        assert_eq!(blocks.len(), 1);
        assert!(
            matches!(blocks[0], Block::Heading(1, _, _)),
            "expected Heading(1), got {:?}",
            blocks[0]
        );
    }

    #[test]
    fn heading_suppressed_when_emit_heading_blocks_false() {
        let para = text_paragraph("Title", true, Some(1));
        let doc = empty_doc(vec![OdfBodyChild::Heading(para)]);
        let opts =
            OdtImportOptions { emit_heading_blocks: false, ..options() };
        let (result, _) = map_document(
            &doc,
            &empty_stylesheet(),
            None,
            &HashMap::new(),
            &opts,
        );
        let blocks = &result.sections[0].blocks;
        assert_eq!(blocks.len(), 1);
        assert!(
            matches!(blocks[0], Block::StyledPara(_)),
            "expected StyledPara, got {:?}",
            blocks[0]
        );
    }

    #[test]
    fn paragraph_is_emitted_as_styled_para() {
        let para = text_paragraph("Hello", false, None);
        let doc = empty_doc(vec![OdfBodyChild::Paragraph(para)]);
        let (result, _) = map_document(
            &doc,
            &empty_stylesheet(),
            None,
            &HashMap::new(),
            &options(),
        );
        let blocks = &result.sections[0].blocks;
        assert!(
            matches!(blocks[0], Block::StyledPara(_)),
            "expected StyledPara, got {:?}",
            blocks[0]
        );
    }

    #[test]
    fn text_content_preserved_in_heading() {
        let para = text_paragraph("Introduction", true, Some(1));
        let doc = empty_doc(vec![OdfBodyChild::Heading(para)]);
        let (result, _) = map_document(
            &doc,
            &empty_stylesheet(),
            None,
            &HashMap::new(),
            &options(),
        );
        if let Block::Heading(_, _, inlines) = &result.sections[0].blocks[0] {
            assert_eq!(inlines.len(), 1);
            assert!(matches!(&inlines[0], loki_doc_model::Inline::Str(s) if s == "Introduction"));
        } else {
            panic!("expected Heading");
        }
    }

    #[test]
    fn meta_title_mapped() {
        let odf_meta = OdfMeta {
            title: Some("My Document".into()),
            creator: Some("Alice".into()),
            ..Default::default()
        };
        let doc = empty_doc(vec![]);
        let (result, _) = map_document(
            &doc,
            &empty_stylesheet(),
            Some(&odf_meta),
            &HashMap::new(),
            &options(),
        );
        assert_eq!(result.meta.title.as_deref(), Some("My Document"));
        assert_eq!(
            result.meta.last_modified_by.as_deref(),
            Some("Alice")
        );
    }

    #[test]
    fn parse_datetime_rfc3339() {
        let dt = parse_datetime("2024-06-15T12:30:00Z");
        assert!(dt.is_some());
    }

    #[test]
    fn parse_datetime_no_tz() {
        let dt = parse_datetime("2024-06-15T12:30:00");
        assert!(dt.is_some());
    }

    #[test]
    fn parse_datetime_invalid_returns_none() {
        let dt = parse_datetime("not-a-date");
        assert!(dt.is_none());
    }
}
