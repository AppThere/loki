// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 AppThere Loki contributors

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
    Block, Caption, ListAttributes, ListDelimiter, ListNumberStyle, StyledParagraph,
    TableOfContentsBlock,
};
use loki_doc_model::content::field::types::{CrossRefFormat, Field, FieldKind};
use loki_doc_model::content::inline::{BookmarkKind, Inline, LinkTarget, NoteKind, StyledRun};
use loki_doc_model::content::table::col::{ColAlignment, ColSpec, ColWidth};
use loki_doc_model::content::table::core::{Table, TableBody, TableCaption, TableFoot, TableHead};
use loki_doc_model::content::table::row::{Cell, Row};
use loki_doc_model::document::Document;
use loki_doc_model::layout::header_footer::{HeaderFooter, HeaderFooterKind};
use loki_doc_model::layout::page::{PageLayout, PageMargins, PageOrientation, PageSize};
use loki_doc_model::layout::section::Section;
use loki_doc_model::meta::core::DocumentMeta;
use loki_doc_model::style::catalog::{StyleCatalog, StyleId};
use loki_doc_model::style::list_style::{ListId, ListLevelKind, NumberingScheme};
use loki_primitives::units::Points;

use crate::error::OdfWarning;
use crate::odt::import::OdtImportOptions;
use crate::odt::mapper::lists::map_list_styles;
use crate::odt::mapper::props::map_cell_props;
use crate::odt::mapper::styles::map_stylesheet;
use crate::odt::model::document::{
    OdfBodyChild, OdfDocument, OdfList, OdfListItem, OdfListItemChild, OdfMasterPage, OdfMeta,
    OdfPageLayout, OdfSection, OdfTableOfContent,
};
use crate::odt::model::fields::OdfField;
use crate::odt::model::frames::{OdfFrame, OdfFrameKind};
use crate::odt::model::notes::{OdfNote, OdfNoteClass};
use crate::odt::model::paragraph::{OdfHyperlink, OdfParagraph, OdfParagraphChild, OdfSpan};
use crate::odt::model::styles::{OdfCellProps, OdfStyle, OdfStylesheet};
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
    /// Column widths from `style:table-column-properties`: style name → points.
    /// Pre-built from the ODF stylesheet before the mapping pass.
    pub col_style_widths: &'a HashMap<String, Points>,
    /// Cell properties from `style:table-cell-properties`: style name → props.
    /// Pre-built from the ODF stylesheet before the mapping pass.
    pub cell_style_props: &'a HashMap<String, OdfCellProps>,
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

    // ── 2. Pre-build column-width lookup from table-column styles ────────────────
    let col_style_widths: HashMap<String, Points> = stylesheet
        .named_styles
        .iter()
        .chain(stylesheet.auto_styles.iter())
        .filter_map(|s| {
            let width_str = s.col_width.as_deref()?;
            let pts = parse_length(width_str)?;
            Some((s.name.clone(), pts))
        })
        .collect();

    // ── 2b. Pre-build cell-style lookup from table-cell styles ───────────────
    let cell_style_props: HashMap<String, OdfCellProps> = stylesheet
        .named_styles
        .iter()
        .chain(stylesheet.auto_styles.iter())
        .filter_map(|s| Some((s.name.clone(), s.cell_props.clone()?)))
        .collect();

    // ── 3. Build style lookup for master page resolution ─────────────────────
    let all_styles: HashMap<&str, &OdfStyle> = stylesheet
        .named_styles
        .iter()
        .chain(stylesheet.auto_styles.iter())
        .map(|s| (s.name.as_str(), s))
        .collect();

    // Identify the initial master page name ("Standard" > "Default" > first).
    let initial_master: Option<&str> = stylesheet
        .master_pages
        .iter()
        .find(|m| m.name == "Standard" || m.name == "Default")
        .or_else(|| stylesheet.master_pages.first())
        .map(|m| m.name.as_str());

    // ── 4. Map body, detecting master page transitions → multiple sections ────
    let (sections, warnings) = {
        let mut ctx = OdfMappingContext {
            styles: &catalog,
            images,
            options,
            col_style_widths: &col_style_widths,
            cell_style_props: &cell_style_props,
            warnings: Vec::new(),
            pending_figures: Vec::new(),
        };

        let mut current_master: Option<String> = initial_master.map(str::to_string);
        let mut current_blocks: Vec<Block> = Vec::new();
        let mut sections: Vec<Section> = Vec::new();

        for child in &doc.body_children {
            // Only paragraphs/headings carry style:master-page-name.
            let new_master = match child {
                OdfBodyChild::Paragraph(para) | OdfBodyChild::Heading(para) => para
                    .style_name
                    .as_deref()
                    .and_then(|sn| resolve_master_page_name(sn, &all_styles)),
                _ => None,
            };

            // Emit a section break only when the master page actually changes.
            if let Some(ref nm) = new_master
                && Some(nm.as_str()) != current_master.as_deref()
            {
                let layout =
                    resolve_page_layout_by_name(stylesheet, current_master.as_deref(), &mut ctx);
                sections.push(Section::with_layout_and_blocks(
                    layout,
                    std::mem::take(&mut current_blocks),
                ));
                current_master = Some(nm.clone());
            }

            if let Some(block) = map_body_child(child, &mut ctx) {
                current_blocks.push(block);
                let figs = std::mem::take(&mut ctx.pending_figures);
                current_blocks.extend(figs);
            }
        }

        // Flush the final (or only) section.
        let layout = resolve_page_layout_by_name(stylesheet, current_master.as_deref(), &mut ctx);
        sections.push(Section::with_layout_and_blocks(layout, current_blocks));

        (sections, ctx.warnings)
    };

    // ── 5. Map metadata ───────────────────────────────────────────────────────
    let doc_meta = meta.map(map_meta).unwrap_or_default();

    // ── 6. Build document (caller sets source) ────────────────────────────────
    let document = Document {
        meta: doc_meta,
        styles: catalog,
        sections,
        settings: None,
        source: None,
    };

    (document, warnings)
}

// ── Body ───────────────────────────────────────────────────────────────────────

/// Convert a slice of [`OdfBodyChild`]s into [`Block`]s, flushing any
/// pending floating figures after each block.
fn map_body_children(children: &[OdfBodyChild], ctx: &mut OdfMappingContext<'_>) -> Vec<Block> {
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

fn map_body_child(child: &OdfBodyChild, ctx: &mut OdfMappingContext<'_>) -> Option<Block> {
    match child {
        OdfBodyChild::Paragraph(para) | OdfBodyChild::Heading(para) => {
            Some(map_paragraph(para, ctx))
        }
        OdfBodyChild::List(list) => Some(map_list(list, ctx)),
        OdfBodyChild::Table(table) => Some(map_table(table, ctx)),
        OdfBodyChild::TableOfContent(toc) => Some(map_toc(toc, ctx)),
        OdfBodyChild::Section(section) => Some(map_section(section, ctx)),
        OdfBodyChild::Other { element } => {
            ctx.warnings.push(OdfWarning::UnrecognisedElement {
                element: element.clone(),
                context: "body index block (unimplemented)".to_string(),
            });
            None
        }
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

fn map_inline(child: &OdfParagraphChild, ctx: &mut OdfMappingContext<'_>) -> Option<Inline> {
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
        OdfParagraphChild::SoftReturn | OdfParagraphChild::Other => None,
        OdfParagraphChild::Tab => Some(Inline::Str("\t".into())),
        OdfParagraphChild::Space { count } => Some(Inline::Str(" ".repeat(*count as usize))),
        OdfParagraphChild::LineBreak => Some(Inline::LineBreak),
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
        OdfField::Date { data_style, .. } => FieldKind::Date {
            format: data_style.clone(),
        },
        OdfField::Time { data_style, .. } => FieldKind::Time {
            format: data_style.clone(),
        },
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
            FieldKind::CrossReference {
                target: ref_name.clone(),
                format,
            }
        }
        OdfField::ChapterName { display_levels } => FieldKind::Raw {
            instruction: format!("chapter display-levels={display_levels}"),
        },
        OdfField::Unknown { local_name, .. } => FieldKind::Raw {
            instruction: local_name.clone(),
        },
    };
    Field {
        kind,
        current_value: None,
        extensions: ExtensionBag::default(),
    }
}

// ── Frames ─────────────────────────────────────────────────────────────────────

/// Map an ODF drawing frame to an inline element.
///
/// For `as-char`-anchored frames, the mapped element is returned directly.
/// For floating frames, a [`Block::Figure`] or [`Block::Div`] is pushed to
/// [`OdfMappingContext::pending_figures`] and `None` is returned.
fn map_frame(frame: &OdfFrame, ctx: &mut OdfMappingContext<'_>) -> Option<Inline> {
    use base64::Engine as _;
    let is_as_char = frame.anchor_type.as_deref() == Some("as-char");

    match &frame.kind {
        OdfFrameKind::Image {
            href,
            media_type,
            title,
            desc,
        } => {
            if !ctx.options.embed_images {
                return None;
            }
            let Some((stored_mt, bytes)) = ctx.images.get(href.as_str()) else {
                ctx.warnings
                    .push(OdfWarning::MissingImage { href: href.clone() });
                return None;
            };
            let b64 = base64::engine::general_purpose::STANDARD.encode(bytes);
            let mt = media_type.as_deref().unwrap_or(stored_mt.as_str());
            let data_uri = format!("data:{mt};base64,{b64}");
            let alt: Vec<Inline> = desc
                .as_deref()
                .or(title.as_deref())
                .map(|s| vec![Inline::Str(s.into())])
                .unwrap_or_default();
            let img = Inline::Image(NodeAttr::default(), alt, LinkTarget::new(data_uri));
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
        OdfFrameKind::Other => {
            ctx.warnings.push(OdfWarning::DroppedFrame {
                name: frame.name.clone(),
            });
            None
        }
    }
}

// ── Lists ──────────────────────────────────────────────────────────────────────

fn map_list(list: &OdfList, ctx: &mut OdfMappingContext<'_>) -> Block {
    let ordered = is_ordered_list(list.style_name.as_deref(), ctx.styles);
    let items: Vec<Vec<Block>> = list
        .items
        .iter()
        .map(|item| map_list_item(item, ctx))
        .collect();

    if ordered {
        let mut attrs = build_list_attributes(list, ctx.styles);
        // Per-item start_value on the first item overrides the style-level start.
        // text:continue-numbering is not tracked across lists (no cross-list state),
        // so only the explicit start_value override is applied here.
        if let Some(first_start) = list.items.first().and_then(|i| i.start_value) {
            attrs.start_number = first_start.cast_signed();
        }
        Block::OrderedList(attrs, items)
    } else {
        Block::BulletList(items)
    }
}

fn map_list_item(item: &OdfListItem, ctx: &mut OdfMappingContext<'_>) -> Vec<Block> {
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
    let Some(name) = style_name else { return false };
    let Some(ls) = catalog.list_styles.get(&ListId::new(name)) else {
        return false;
    };
    ls.levels
        .first()
        .is_some_and(|l| matches!(l.kind, ListLevelKind::Numbered { .. }))
}

/// Build [`ListAttributes`] from the first level of the named list style.
fn build_list_attributes(list: &OdfList, catalog: &StyleCatalog) -> ListAttributes {
    let default = ListAttributes::default();
    let Some(name) = list.style_name.as_deref() else {
        return default;
    };
    let Some(ls) = catalog.list_styles.get(&ListId::new(name)) else {
        return default;
    };
    let Some(first) = ls.levels.first() else {
        return default;
    };
    match &first.kind {
        ListLevelKind::Numbered {
            scheme,
            start_value,
            format,
            ..
        } => {
            let style = match scheme {
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
                start_number: (*start_value).cast_signed(),
                style,
                delimiter,
            }
        }
        _ => default,
    }
}

// ── Tables ─────────────────────────────────────────────────────────────────────

fn map_table(table: &OdfTable, ctx: &mut OdfMappingContext<'_>) -> Block {
    // COMPAT(odf): column width from style:table-column-properties
    // Expand repeated column definitions, resolving fixed widths from style lookup.
    let col_specs: Vec<ColSpec> = table
        .col_defs
        .iter()
        .flat_map(|def| {
            let count = def.columns_repeated.max(1) as usize;
            let width = def
                .style_name
                .as_deref()
                .and_then(|name| ctx.col_style_widths.get(name))
                .map_or(ColWidth::Proportional(1.0), |&pts| ColWidth::Fixed(pts));
            let spec = ColSpec {
                alignment: ColAlignment::Default,
                width,
            };
            std::iter::repeat_n(spec, count)
        })
        .collect();

    let body_rows: Vec<Row> = table
        .rows
        .iter()
        .map(|odf_row| {
            let cells: Vec<Cell> = odf_row
                .cells
                .iter()
                .filter_map(|odf_cell| {
                    // Covered cells are suppressed; the spanning cell carries
                    // `row_span` from `table:number-rows-spanned` (read by the reader).
                    if odf_cell.is_covered {
                        return None;
                    }
                    let blocks: Vec<Block> = odf_cell
                        .paragraphs
                        .iter()
                        .flat_map(|p| {
                            let block = map_paragraph(p, ctx);
                            let figs = std::mem::take(&mut ctx.pending_figures);
                            std::iter::once(block).chain(figs)
                        })
                        .collect();
                    // NOTE: ODF cell properties are mapped to the same CellProps
                    // type as OOXML. The layout engine applies them identically.
                    let props = odf_cell
                        .style_name
                        .as_deref()
                        .and_then(|n| ctx.cell_style_props.get(n))
                        .map(map_cell_props)
                        .unwrap_or_default();
                    Some(Cell {
                        attr: NodeAttr::default(),
                        alignment: ColAlignment::Default,
                        row_span: odf_cell.row_span,
                        col_span: odf_cell.col_span,
                        blocks,
                        props,
                    })
                })
                .collect();
            Row::new(cells)
        })
        .collect();

    Block::Table(Box::new(Table {
        attr: NodeAttr::default(),
        caption: TableCaption::default(),
        width: None,
        col_specs,
        head: TableHead::empty(),
        bodies: vec![TableBody::from_rows(body_rows)],
        foot: TableFoot::empty(),
    }))
}

// ── Table of contents ──────────────────────────────────────────────────────────

fn map_toc(toc: &OdfTableOfContent, ctx: &mut OdfMappingContext<'_>) -> Block {
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

fn map_section(section: &OdfSection, ctx: &mut OdfMappingContext<'_>) -> Block {
    let blocks = map_body_children(&section.children, ctx);
    Block::Div(NodeAttr::default(), blocks)
}

// ── Page layout ────────────────────────────────────────────────────────────────

/// Resolves the effective master page name for a paragraph style, following
/// the `style:parent-style-name` inheritance chain.
///
/// Returns `None` when no master page transition is defined anywhere in the
/// chain. A cycle in the parent chain terminates the walk without a result.
fn resolve_master_page_name<'a>(
    style_name: &str,
    all_styles: &'a HashMap<&str, &'a OdfStyle>,
) -> Option<String> {
    let mut current = style_name;
    let mut depth = 0usize;
    loop {
        // Guard against malformed cycles in the style inheritance chain.
        if depth > 32 {
            break;
        }
        depth += 1;
        let style = all_styles.get(current)?;
        if let Some(ref mpn) = style.master_page_name
            && !mpn.is_empty()
        {
            return Some(mpn.clone());
        }
        current = style.parent_name.as_deref()?;
    }
    None
}

/// Build a [`PageLayout`] for the named master page.
///
/// Looks up the named master page in `stylesheet.master_pages`. If
/// `master_name` is `None`, falls back to the "Standard" / "Default" master,
/// then the first one. Converts the associated `style:page-layout` to a
/// format-neutral [`PageLayout`] and populates all header/footer variants.
/// Returns [`PageLayout::default`] when no master page is found.
fn resolve_page_layout_by_name(
    stylesheet: &OdfStylesheet,
    master_name: Option<&str>,
    ctx: &mut OdfMappingContext<'_>,
) -> PageLayout {
    let master = master_name
        .and_then(|name| stylesheet.master_pages.iter().find(|m| m.name == name))
        .or_else(|| {
            stylesheet
                .master_pages
                .iter()
                .find(|m| m.name == "Standard" || m.name == "Default")
        })
        .or_else(|| stylesheet.master_pages.first());

    let odf_layout = master.and_then(|m| {
        stylesheet
            .page_layouts
            .iter()
            .find(|pl| pl.name == m.page_layout_name)
    });

    let mut layout = match odf_layout {
        Some(pl) => convert_page_layout(pl),
        None => PageLayout::default(),
    };

    if let Some(master) = master {
        apply_master_page_hf(master, &mut layout, ctx);
    }

    layout
}

/// Map all header/footer variants from `master` onto `layout`.
fn apply_master_page_hf(
    master: &OdfMasterPage,
    layout: &mut PageLayout,
    ctx: &mut OdfMappingContext<'_>,
) {
    layout.header = map_hf_paras(master.header.as_ref(), HeaderFooterKind::Default, ctx);
    layout.footer = map_hf_paras(master.footer.as_ref(), HeaderFooterKind::Default, ctx);
    layout.header_first = map_hf_paras(master.header_first.as_ref(), HeaderFooterKind::First, ctx);
    layout.footer_first = map_hf_paras(master.footer_first.as_ref(), HeaderFooterKind::First, ctx);
    layout.header_even = map_hf_paras(master.header_even.as_ref(), HeaderFooterKind::Even, ctx);
    layout.footer_even = map_hf_paras(master.footer_even.as_ref(), HeaderFooterKind::Even, ctx);
}

/// Convert a list of [`OdfParagraph`]s into a [`HeaderFooter`].
///
/// Returns `None` when `paras` is `None` or empty (preserving the "absent
/// variant" semantics that [`assign_headers_footers`] relies on).
///
/// [`assign_headers_footers`]: loki_layout::flow::assign_headers_footers
fn map_hf_paras(
    paras: Option<&Vec<OdfParagraph>>,
    kind: HeaderFooterKind,
    ctx: &mut OdfMappingContext<'_>,
) -> Option<HeaderFooter> {
    let paras = paras?;
    if paras.is_empty() {
        return None;
    }
    let blocks = paras.iter().map(|p| map_paragraph(p, ctx)).collect();
    Some(HeaderFooter { kind, blocks })
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
    let keywords = if meta.keywords.is_empty() {
        None
    } else {
        Some(meta.keywords.join(", "))
    };
    DocumentMeta {
        title: meta.title.clone(),
        subject: meta.subject.clone(),
        keywords,
        description: meta.description.clone(),
        creator: meta.initial_creator.clone(),
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
        let (result, warnings) =
            map_document(&doc, &empty_stylesheet(), None, &HashMap::new(), &options());
        assert!(warnings.is_empty());
        assert_eq!(result.sections.len(), 1);
        assert!(result.sections[0].blocks.is_empty());
    }

    #[test]
    fn heading_is_emitted_as_heading_block() {
        let para = text_paragraph("Title", true, Some(1));
        let doc = empty_doc(vec![OdfBodyChild::Heading(para)]);
        let (result, _) =
            map_document(&doc, &empty_stylesheet(), None, &HashMap::new(), &options());
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
        let opts = OdtImportOptions {
            emit_heading_blocks: false,
            ..options()
        };
        let (result, _) = map_document(&doc, &empty_stylesheet(), None, &HashMap::new(), &opts);
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
        let (result, _) =
            map_document(&doc, &empty_stylesheet(), None, &HashMap::new(), &options());
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
        let (result, _) =
            map_document(&doc, &empty_stylesheet(), None, &HashMap::new(), &options());
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
            subject: Some("My Subject".into()),
            creator: Some("Alice".into()),
            initial_creator: Some("Bob".into()),
            keywords: vec!["k1".into(), "k2".into()],
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
        assert_eq!(result.meta.subject.as_deref(), Some("My Subject"));
        assert_eq!(result.meta.last_modified_by.as_deref(), Some("Alice"));
        assert_eq!(result.meta.creator.as_deref(), Some("Bob"));
        assert_eq!(result.meta.keywords.as_deref(), Some("k1, k2"));
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

    // ── resolve_master_page_name unit tests ───────────────────────────────────

    fn style_with_mpn(name: &str, mpn: Option<&str>, parent: Option<&str>) -> OdfStyle {
        use crate::odt::model::styles::OdfStyleFamily;
        OdfStyle {
            name: name.into(),
            display_name: None,
            family: OdfStyleFamily::Paragraph,
            parent_name: parent.map(String::from),
            list_style_name: None,
            para_props: None,
            text_props: None,
            col_width: None,
            cell_props: None,
            is_automatic: false,
            master_page_name: mpn.map(String::from),
        }
    }

    fn make_lookup<'a>(styles: &'a [OdfStyle]) -> HashMap<&'a str, &'a OdfStyle> {
        styles.iter().map(|s| (s.name.as_str(), s)).collect()
    }

    /// Direct `master_page_name` on the style is returned.
    #[test]
    fn resolve_mpn_direct() {
        let styles = [style_with_mpn("LandscapeStyle", Some("Landscape"), None)];
        let lookup = make_lookup(&styles);
        assert_eq!(
            resolve_master_page_name("LandscapeStyle", &lookup),
            Some("Landscape".into())
        );
    }

    /// When the style has no `master_page_name` but its parent does, the
    /// parent's value is returned.
    #[test]
    fn resolve_mpn_inherited_from_parent() {
        let styles = [
            style_with_mpn("Base", Some("Landscape"), None),
            style_with_mpn("Child", None, Some("Base")),
        ];
        let lookup = make_lookup(&styles);
        assert_eq!(
            resolve_master_page_name("Child", &lookup),
            Some("Landscape".into())
        );
    }

    /// An empty `master_page_name` string is treated as absent — `None` returned.
    #[test]
    fn resolve_mpn_empty_string_returns_none() {
        let styles = [style_with_mpn("PlainStyle", Some(""), None)];
        let lookup = make_lookup(&styles);
        assert_eq!(resolve_master_page_name("PlainStyle", &lookup), None);
    }

    /// A style with no master page anywhere in the chain returns `None`.
    #[test]
    fn resolve_mpn_no_master_page_in_chain() {
        let styles = [
            style_with_mpn("Root", None, None),
            style_with_mpn("Child", None, Some("Root")),
        ];
        let lookup = make_lookup(&styles);
        assert_eq!(resolve_master_page_name("Child", &lookup), None);
    }

    /// A style that doesn't exist in the lookup returns `None` without panicking.
    #[test]
    fn resolve_mpn_unknown_style_returns_none() {
        let styles: [OdfStyle; 0] = [];
        let lookup = make_lookup(&styles);
        assert_eq!(resolve_master_page_name("NonExistent", &lookup), None);
    }
}
