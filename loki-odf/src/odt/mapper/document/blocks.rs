// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 AppThere Loki contributors

//! Block-level mapping for lists, tables, tables of contents, and sections.

use loki_doc_model::content::attr::NodeAttr;
use loki_doc_model::content::block::{Block, TableOfContentsBlock};
use loki_doc_model::content::table::col::{ColAlignment, ColSpec, ColWidth};
use loki_doc_model::content::table::core::{Table, TableBody, TableCaption, TableFoot, TableHead};
use loki_doc_model::content::table::row::{Cell, Row};
use loki_doc_model::style::catalog::StyleCatalog;
use loki_doc_model::style::list_style::{ListId, ListLevelKind};

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

/// Deepest list level the model addresses (levels are `0..=8`, matching the
/// nine-level `ListStyle` definitions and both export writers).
const MAX_LIST_LEVEL: u8 = 8;

/// Maps a `<text:list>` tree onto the modern flat representation (§10 path
/// A): one `StyledPara` per item paragraph, carrying `list_id` +
/// `list_level`, nested `<text:list>` elements becoming deeper levels of the
/// same run. This retires the legacy pandoc `BulletList`/`OrderedList`
/// emission, whose items were read-only in the editor (no `PathStep`
/// addressed them) and whose rendering hardcoded level-0 markers.
///
/// The list style reference is `text:style-name`, which the catalog already
/// holds as a [`ListStyle`](loki_doc_model::style::list_style::ListStyle)
/// (read by `styles_list_style`). A list with no resolvable style falls back
/// to the built-in default of its kind — seeded into the catalog after
/// mapping via the context's `needs_default_*` flags, so the reference is
/// never dangling.
///
/// The model's start value lives in the shared style, not per item, so a
/// `text:start-value` on the **first** item (the "this list starts at N"
/// idiom) is preserved by synthesizing a derived style — the base style with
/// that level's start replaced, under a derived id — collected on the
/// context and merged into the catalog after mapping. A restart on a *later*
/// item (mid-list renumbering) has no model expression and is dropped.
pub(super) fn map_list(list: &OdfList, level: u8, ctx: &mut OdfMappingContext<'_>) -> Vec<Block> {
    let mut list_id = resolve_list_id(list.style_name.as_deref(), ctx);
    if let Some(start) = list.items.first().and_then(|i| i.start_value)
        && let Some(derived) = derive_start_override(&list_id, level, start, ctx)
    {
        list_id = derived;
    }
    let mut blocks = Vec::new();
    for item in &list.items {
        map_list_item(item, &list_id, level, ctx, &mut blocks);
    }
    blocks
}

/// Synthesizes (or reuses) a derived list style whose `level` starts at
/// `start`, returning its id — `None` when the base style is not in the
/// catalog or the level does not exist or is not numbered.
fn derive_start_override(
    base_id: &str,
    level: u8,
    start: u32,
    ctx: &mut OdfMappingContext<'_>,
) -> Option<String> {
    let derived_id = format!("{base_id}-start{start}-l{level}");
    if ctx
        .synthesized_list_styles
        .iter()
        .any(|s| s.id.as_str() == derived_id)
    {
        return Some(derived_id);
    }
    let base = ctx.styles.list_styles.get(&ListId::new(base_id))?;
    let mut derived = base.clone();
    let lvl = derived.levels.get_mut(usize::from(level))?;
    let ListLevelKind::Numbered { start_value, .. } = &mut lvl.kind else {
        return None;
    };
    if *start_value == start {
        return None; // the base already starts there — no derivation needed
    }
    *start_value = start;
    derived.id = ListId::new(&derived_id);
    derived.display_name = Some(format!("{base_id} (start {start})"));
    ctx.synthesized_list_styles.push(derived);
    Some(derived_id)
}

/// The catalog id this list's items reference: its own named style when the
/// catalog defines it, else the default style of its kind (flagging the
/// context so the default gets seeded).
fn resolve_list_id(style_name: Option<&str>, ctx: &mut OdfMappingContext<'_>) -> String {
    if let Some(name) = style_name
        && ctx.styles.list_styles.contains_key(&ListId::new(name))
    {
        return name.to_string();
    }
    if is_ordered_list(style_name, ctx.styles) {
        ctx.needs_default_numbered = true;
        loki_doc_model::style::list_defaults::DEFAULT_NUMBERED_LIST_ID.to_string()
    } else {
        ctx.needs_default_bullet = true;
        loki_doc_model::style::list_defaults::DEFAULT_BULLET_LIST_ID.to_string()
    }
}

fn map_list_item(
    item: &OdfListItem,
    list_id: &str,
    level: u8,
    ctx: &mut OdfMappingContext<'_>,
    blocks: &mut Vec<Block>,
) {
    for child in &item.children {
        match child {
            OdfListItemChild::Paragraph(p) | OdfListItemChild::Heading(p) => {
                let block = with_list_membership(map_paragraph(p, ctx), list_id, level);
                blocks.push(block);
                let figs = std::mem::take(&mut ctx.pending_figures);
                blocks.extend(figs);
            }
            OdfListItemChild::List(nested) => {
                let deeper = (level + 1).min(MAX_LIST_LEVEL);
                blocks.extend(map_list(nested, deeper, ctx));
                let figs = std::mem::take(&mut ctx.pending_figures);
                blocks.extend(figs);
            }
        }
    }
}

/// Attaches `list_id`/`list_level` to a mapped item paragraph. A plain
/// paragraph is promoted to a `StyledPara`; an already-styled one keeps its
/// style and gains the list props. A heading keeps its heading identity and
/// stays out of the list (heading-ness carries more: outline level, TOC).
fn with_list_membership(block: Block, list_id: &str, level: u8) -> Block {
    let set = |props: &mut loki_doc_model::style::props::para_props::ParaProps| {
        props.list_id = Some(ListId::new(list_id));
        props.list_level = Some(level);
    };
    match block {
        Block::Para(inlines) | Block::Plain(inlines) => {
            let mut props = loki_doc_model::style::props::para_props::ParaProps::default();
            set(&mut props);
            Block::StyledPara(loki_doc_model::content::block::StyledParagraph {
                style_id: None,
                direct_para_props: Some(Box::new(props)),
                direct_char_props: None,
                inlines,
                attr: NodeAttr::default(),
            })
        }
        Block::StyledPara(mut sp) => {
            let mut props = sp.direct_para_props.take().unwrap_or_default();
            set(&mut props);
            sp.direct_para_props = Some(props);
            Block::StyledPara(sp)
        }
        other => other,
    }
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

    let mut mapped = Table {
        attr: NodeAttr::default(),
        caption: TableCaption::default(),
        width: None,
        col_specs,
        head: TableHead::empty(),
        bodies: vec![TableBody::from_rows(body_rows)],
        foot: TableFoot::empty(),
    };
    // Preserve the table's named-style reference (`table:style-name`).
    mapped.set_style_name(table.style_name.clone());
    Block::Table(Box::new(mapped))
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
