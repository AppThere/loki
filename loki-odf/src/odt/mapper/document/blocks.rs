// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 AppThere Loki contributors

//! Block-level mapping for lists, tables, tables of contents, and sections.

use loki_doc_model::content::attr::NodeAttr;
use loki_doc_model::content::block::{
    Block, ListAttributes, ListDelimiter, ListNumberStyle, TableOfContentsBlock,
};
use loki_doc_model::content::table::col::{ColAlignment, ColSpec, ColWidth};
use loki_doc_model::content::table::core::{Table, TableBody, TableCaption, TableFoot, TableHead};
use loki_doc_model::content::table::row::{Cell, Row};
use loki_doc_model::style::catalog::StyleCatalog;
use loki_doc_model::style::list_style::{ListId, ListLevelKind, NumberingScheme};

use crate::limits::MAX_TABLE_COLUMNS;
use crate::odt::mapper::props::map_cell_props;
use crate::odt::model::document::{
    OdfList, OdfListItem, OdfListItemChild, OdfSection, OdfTableOfContent,
};
use crate::odt::model::tables::OdfTable;

use super::OdfMappingContext;
use super::inlines::map_paragraph;
use super::map_body_children;

// ── Lists ──────────────────────────────────────────────────────────────────────

pub(super) fn map_list(list: &OdfList, ctx: &mut OdfMappingContext<'_>) -> Block {
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

pub(super) fn map_table(table: &OdfTable, ctx: &mut OdfMappingContext<'_>) -> Block {
    // COMPAT(odf): column width from style:table-column-properties
    // Expand repeated column definitions, resolving fixed widths from style lookup.
    let col_specs: Vec<ColSpec> = table
        .col_defs
        .iter()
        .flat_map(|def| {
            // Clamp attacker-controlled number-columns-repeated so a single
            // table:table-column cannot expand into billions of ColSpecs.
            let count = def.columns_repeated.clamp(1, MAX_TABLE_COLUMNS) as usize;
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
                    // Ordered block content (paragraphs, lists, and nested tables)
                    // maps recursively through `map_body_children`, which dispatches
                    // a nested `table:table` back through `map_table` — so a table
                    // inside a cell becomes a `Block::Table` inside the cell, in
                    // document order with any sibling paragraphs.
                    let blocks: Vec<Block> = map_body_children(&odf_cell.content, ctx);
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

pub(super) fn map_toc(toc: &OdfTableOfContent, ctx: &mut OdfMappingContext<'_>) -> Block {
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

pub(super) fn map_section(section: &OdfSection, ctx: &mut OdfMappingContext<'_>) -> Block {
    let blocks = map_body_children(&section.children, ctx);
    Block::Div(NodeAttr::default(), blocks)
}
