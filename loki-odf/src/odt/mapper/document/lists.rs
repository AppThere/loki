// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 AppThere Loki contributors

//! ODF list → [`Block`] mapping.

use loki_doc_model::content::block::{Block, ListAttributes, ListDelimiter, ListNumberStyle};
use loki_doc_model::style::catalog::StyleCatalog;
use loki_doc_model::style::list_style::{ListId, ListLevelKind, NumberingScheme};

use crate::odt::model::document::{OdfList, OdfListItem, OdfListItemChild};

use super::context::OdfMappingContext;
use super::paragraphs::map_paragraph;

pub(crate) fn map_list(list: &OdfList, ctx: &mut OdfMappingContext<'_>) -> Block {
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
