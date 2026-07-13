// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 AppThere Loki contributors

//! Paragraph between-border resolution (fidelity gap #26).
//!
//! Word semantics (OOXML `w:pBdr/w:between`, ECMA-376 §17.3.1.4): consecutive
//! paragraphs with *identical* border settings form one bordered group — the
//! outer top edge is drawn on the first member only, the outer bottom on the
//! last, and the `between` rule is drawn once at each internal boundary.
//! Without this, adjacent same-bordered paragraphs each draw their own full
//! box (a doubled line at every boundary) and `w:between` is ignored.
//!
//! [`stage`] is called by the block loops in `flow.rs` for the block about to
//! flow; it decides the current paragraph's group membership by comparing a
//! cheap 5-edge border signature with its slice neighbours (a light child-wins
//! chain probe like `para_keep_with_next` — no full `ResolvedParaProps`
//! resolve). The result rides `FlowState::staged_between` and is consumed by
//! `flow_paragraph`, overriding the resolved top/bottom edges *before* layout
//! so the adjustment participates in the paragraph-cache key.
//!
//! Scope: `Block::StyledPara` runs within one block slice (top level, or one
//! nested container). Keep-with-next chains and synthesized paragraphs
//! (headings, plain paras) break a group, matching their bypass of `stage`.

use loki_doc_model::content::block::{Block, StyledParagraph};
use loki_doc_model::style::catalog::{MAX_STYLE_CHAIN_DEPTH, StyleCatalog};
use loki_doc_model::style::props::border::Border;
use loki_doc_model::style::props::para_props::ParaProps;

use crate::items::BorderEdge;
use crate::resolve::convert_border;

/// Border edge replacement for the paragraph being flowed, staged on
/// `FlowState` by [`stage`] and consumed (taken) by `flow_paragraph`.
pub(crate) struct BetweenOverride {
    /// The previous paragraph continues the group: drop this paragraph's top
    /// edge (the boundary rule was already drawn above).
    pub suppress_top: bool,
    /// `Some(edge)` when the next paragraph continues the group: replace the
    /// bottom edge with the group's `between` rule (`None` = no rule defined,
    /// so the boundary draws nothing).
    pub bottom: Option<Option<BorderEdge>>,
}

/// Computes the [`BetweenOverride`] for `blocks[i]`, or `None` when the block
/// is not a bordered paragraph or neither neighbour shares its border group.
pub(crate) fn stage(blocks: &[Block], i: usize, catalog: &StyleCatalog) -> Option<BetweenOverride> {
    let cur = border_sig(blocks.get(i)?, catalog)?;
    let prev_in = i
        .checked_sub(1)
        .and_then(|j| blocks.get(j))
        .and_then(|b| border_sig(b, catalog))
        .is_some_and(|s| s == cur);
    let next_in = blocks
        .get(i + 1)
        .and_then(|b| border_sig(b, catalog))
        .is_some_and(|s| s == cur);
    if !prev_in && !next_in {
        return None;
    }
    let bottom = next_in.then(|| cur.between.as_ref().and_then(convert_border));
    Some(BetweenOverride {
        suppress_top: prev_in,
        bottom,
    })
}

/// The five effective paragraph border settings — the group-membership key.
/// Two paragraphs join one bordered group iff their signatures are equal
/// (Word compares the whole border set, including `between`).
#[derive(Default, PartialEq)]
struct BorderSig {
    top: Option<Border>,
    bottom: Option<Border>,
    left: Option<Border>,
    right: Option<Border>,
    between: Option<Border>,
}

impl BorderSig {
    fn any(&self) -> bool {
        self.top.is_some()
            || self.bottom.is_some()
            || self.left.is_some()
            || self.right.is_some()
            || self.between.is_some()
    }

    /// Child-wins fill: only unset fields take the (more inherited) values.
    fn fill_from(&mut self, p: &ParaProps) {
        macro_rules! fill {
            ($field:ident, $src:ident) => {
                if self.$field.is_none() {
                    self.$field = p.$src.clone();
                }
            };
        }
        fill!(top, border_top);
        fill!(bottom, border_bottom);
        fill!(left, border_left);
        fill!(right, border_right);
        fill!(between, border_between);
    }
}

/// The effective border signature of a paragraph block, or `None` when the
/// block is not a `StyledPara` or carries no border at all. Mirrors the
/// child-wins resolution of `resolve_para_props` (direct formatting first,
/// then the named-style parent chain) for just the five border fields.
fn border_sig(block: &Block, catalog: &StyleCatalog) -> Option<BorderSig> {
    let Block::StyledPara(p) = block else {
        return None;
    };
    let sig = resolve_border_fields(p, catalog);
    sig.any().then_some(sig)
}

fn resolve_border_fields(p: &StyledParagraph, catalog: &StyleCatalog) -> BorderSig {
    let mut sig = BorderSig::default();
    if let Some(direct) = &p.direct_para_props {
        sig.fill_from(direct);
    }
    let mut id = catalog.effective_paragraph_style(p.style_id.as_ref());
    // `..=` covers the starting style plus MAX parents, matching the
    // cyclic-chain truncation of `StyleCatalog::resolve_para`.
    for _ in 0..=MAX_STYLE_CHAIN_DEPTH {
        let Some(style) = id.and_then(|sid| catalog.paragraph_styles.get(sid)) else {
            break;
        };
        sig.fill_from(&style.para_props);
        id = style.parent.as_ref();
    }
    sig
}
