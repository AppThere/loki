// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 AppThere Loki contributors

//! Rendering for §10 path-A lists: consecutive `StyledPara`s carrying
//! `list_id` become nested `<ul>`/`<ol>` markup. Every block *sequence* in
//! the content renderer routes through [`RenderCtx::render_blocks`], so a
//! list run inside a blockquote, cell, or figure nests correctly too; a lone
//! block still uses `render_block` directly.
//!
//! `<ul>` vs `<ol>` comes from the referenced style's level-0 kind, snapshot
//! into [`RenderCtx::numbered_list_ids`] at render start (the ctx does not
//! carry the whole catalog). An id the catalog does not define renders as
//! `<ul>` — the same bullet fallback both format writers use.

use loki_doc_model::content::block::{Block, StyledParagraph};

use crate::content::RenderCtx;

/// The `(list-item paragraph, level)` view of a block, when it is one.
fn as_list_item(block: &Block) -> Option<(&StyledParagraph, u8)> {
    let Block::StyledPara(sp) = block else {
        return None;
    };
    let props = sp.direct_para_props.as_ref()?;
    props
        .list_id
        .as_ref()
        .map(|_| (sp, props.list_level.unwrap_or(0)))
}

/// The list id a block references, when it is a list item.
fn list_id_of(block: &Block) -> Option<&str> {
    let Block::StyledPara(sp) = block else {
        return None;
    };
    sp.direct_para_props
        .as_ref()?
        .list_id
        .as_ref()
        .map(loki_doc_model::style::list_style::ListId::as_str)
}

impl RenderCtx {
    /// Renders a block sequence, grouping consecutive same-list path-A items
    /// into one nested list element.
    pub(crate) fn render_blocks(&mut self, blocks: &[Block], out: &mut String) {
        let mut i = 0;
        while i < blocks.len() {
            let Some(id) = list_id_of(&blocks[i]) else {
                self.render_block(&blocks[i], out);
                i += 1;
                continue;
            };
            let run_end = blocks[i..]
                .iter()
                .position(|b| list_id_of(b) != Some(id))
                .map_or(blocks.len(), |n| i + n);
            let numbered = self.numbered_list_ids.contains(id);
            let items: Vec<(&StyledParagraph, u8)> =
                blocks[i..run_end].iter().filter_map(as_list_item).collect();
            self.render_list_run(&items, numbered, out);
            i = run_end;
        }
    }

    /// One list run as nested `<ul>`/`<ol>`: a deeper item opens a sub-list
    /// inside the previous item's `<li>`; a shallower one closes back down.
    fn render_list_run(
        &mut self,
        items: &[(&StyledParagraph, u8)],
        numbered: bool,
        out: &mut String,
    ) {
        let tag = if numbered { "ol" } else { "ul" };
        // Invariant: `li_open.len()` == number of open lists; `li_open[k]`
        // says whether level k's current `<li>` is still open.
        let mut li_open: Vec<bool> = Vec::new();
        for (sp, level) in items {
            let target = usize::from(*level) + 1;
            while li_open.len() > target {
                if li_open.pop() == Some(true) {
                    out.push_str(&format!("</li>\n</{tag}>\n"));
                } else {
                    out.push_str(&format!("</{tag}>\n"));
                }
                // Closing a nested list re-enters the parent's open <li>.
            }
            if li_open.len() == target && li_open.last() == Some(&true) {
                out.push_str("</li>\n");
                *li_open.last_mut().unwrap_or(&mut false) = false;
            }
            while li_open.len() < target {
                out.push_str(&format!("<{tag}>\n"));
                li_open.push(false);
                // A jump of more than one level needs a container <li> at
                // each intermediate level — a bare list inside a list is
                // invalid XHTML.
                if li_open.len() < target {
                    out.push_str("<li>");
                    if let Some(flag) = li_open.last_mut() {
                        *flag = true;
                    }
                }
            }
            out.push_str("<li>");
            self.render_inlines(&sp.inlines, out);
            if let Some(flag) = li_open.last_mut() {
                *flag = true;
            }
        }
        while !li_open.is_empty() {
            if li_open.pop() == Some(true) {
                out.push_str(&format!("</li>\n</{tag}>\n"));
            } else {
                out.push_str(&format!("</{tag}>\n"));
            }
        }
    }
}
