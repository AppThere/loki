// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 AppThere Loki contributors

//! `Builder`'s inline-stack plumbing and block flushing (split from
//! `convert.rs` for the 300-line ceiling).

use loki_doc_model::content::attr::NodeAttr;
use loki_doc_model::content::block::{Block, StyledParagraph};
use loki_doc_model::content::inline::Inline;
use loki_doc_model::content::table::core::Table;
use loki_doc_model::style::catalog::StyleId;
use loki_doc_model::style::list_defaults::{ensure_default_bullet, ensure_default_numbered};
use loki_doc_model::style::props::para_props::ParaProps;

use super::{Builder, InlineWrap, ListFlavor};

impl Builder {
    // ── Inline plumbing ───────────────────────────────────────────────────

    pub(super) fn push_inline(&mut self, inline: Inline) {
        match self.inline_stack.last_mut() {
            Some((_, buf)) => buf.push(inline),
            None => self.inlines.push(inline),
        }
    }

    pub(super) fn push_wrap(&mut self, wrap: InlineWrap) {
        self.inline_stack.push((wrap, Vec::new()));
    }

    pub(super) fn pop_wrap(&mut self) {
        let Some((wrap, body)) = self.inline_stack.pop() else {
            return;
        };
        let wrapped = match wrap {
            InlineWrap::Emph => Inline::Emph(body),
            InlineWrap::Strong => Inline::Strong(body),
            InlineWrap::Strikeout => Inline::Strikeout(body),
            InlineWrap::Link(target) => Inline::Link(NodeAttr::default(), body, target),
            InlineWrap::Image(target) => Inline::Image(NodeAttr::default(), body, target),
        };
        self.push_inline(wrapped);
    }

    pub(super) fn take_inlines(&mut self) -> Vec<Inline> {
        // An unbalanced wrapper (malformed input) flushes as literal content.
        while !self.inline_stack.is_empty() {
            self.pop_wrap();
        }
        std::mem::take(&mut self.inlines)
    }

    // ── Block flushing ────────────────────────────────────────────────────

    /// Flushes a tight list item's accumulated inlines, if any.
    pub(super) fn flush_tight_item(&mut self) {
        if self.item_pending && !self.inlines.is_empty() && self.inline_stack.is_empty() {
            let inlines = std::mem::take(&mut self.inlines);
            self.item_pending = false;
            self.push_list_item(inlines);
        }
    }

    pub(super) fn flush_paragraph(&mut self) {
        let inlines = self.take_inlines();
        if inlines.is_empty() {
            return;
        }
        // Precedence: a paragraph inside a list item is the item; inside a
        // blockquote it is quoted; otherwise plain.
        if !self.list_stack.is_empty() && self.item_pending {
            self.item_pending = false;
            self.push_list_item(inlines);
            return;
        }
        if self.blockquote_depth > 0 {
            self.blocks.push(Block::StyledPara(StyledParagraph {
                style_id: Some(StyleId::new("Blockquote")),
                direct_para_props: None,
                direct_char_props: None,
                inlines,
                attr: NodeAttr::default(),
            }));
            return;
        }
        self.blocks.push(Block::Para(inlines));
    }

    /// One list item at the current stack depth, on the modern
    /// `StyledPara` + `list_id` representation (§10 path A), with the default
    /// style seeded into the document's catalog.
    pub(super) fn push_list_item(&mut self, inlines: Vec<Inline>) {
        let flavor = *self.list_stack.last().unwrap_or(&ListFlavor::Bullet);
        let id = match flavor {
            ListFlavor::Bullet => ensure_default_bullet(&mut self.doc.styles),
            ListFlavor::Numbered => ensure_default_numbered(&mut self.doc.styles),
        };
        #[allow(clippy::cast_possible_truncation)] // depth is bounded by the clamp
        let level = (self.list_stack.len().saturating_sub(1)).min(8) as u8;
        self.blocks.push(Block::StyledPara(StyledParagraph {
            style_id: None,
            direct_para_props: Some(Box::new(ParaProps {
                list_id: Some(id),
                list_level: Some(level),
                ..Default::default()
            })),
            direct_char_props: None,
            inlines,
            attr: NodeAttr::default(),
        }));
    }

    pub(super) fn flush_table(&mut self) {
        let Some(build) = self.table.take() else {
            return;
        };
        let cols = build.rows.iter().map(Vec::len).max().unwrap_or(0);
        if cols == 0 {
            return;
        }
        let mut table = Table::grid(build.rows.len(), cols);
        for (r, row) in build.rows.into_iter().enumerate() {
            for (c, cell) in row.into_iter().enumerate() {
                table.bodies[0].body_rows[r].cells[c].blocks = vec![Block::Para(cell)];
            }
        }
        self.blocks.push(Block::Table(Box::new(table)));
    }
}
