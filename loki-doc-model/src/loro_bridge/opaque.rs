// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 AppThere Loki contributors

//! Opaque-snapshot preservation for blocks the Loro schema cannot represent.
//!
//! The CRDT schema flattens paragraph content into a [`loro::LoroText`] with
//! marks, which cannot carry most structured content (tables, lists, figures,
//! footnote bodies, fields, math). Before this module existed, such blocks
//! were written as empty stubs and read back as `Block::HorizontalRule` —
//! i.e. a single edit cycle destroyed them.
//!
//! Instead, any block that cannot round-trip through the text schema is
//! serialized to JSON (via the model's `serde` derives) and stored verbatim
//! under [`KEY_OPAQUE_JSON`]. On read it is deserialized back into the exact
//! original [`Block`]. Opaque blocks render normally but are not
//! collaboratively editable from within; converting them to native CRDT
//! mappings is tracked as TODO(loro-bridge) work.
//!
//! Inline objects are migrating off this fallback one variant at a time. A
//! *top-level* inline image ([`MARK_IMAGE`][crate::loro_schema::MARK_IMAGE]) or
//! footnote/endnote ([`MARK_NOTE`][crate::loro_schema::MARK_NOTE]) is now mapped
//! natively — an
//! [`OBJECT_REPLACEMENT_CHAR`][crate::loro_schema::OBJECT_REPLACEMENT_CHAR]
//! anchor carries the object as a mark — so it stays a live, positioned,
//! deletable inline (see `inlines::write_inline_object`). The same object
//! *nested* inside a wrapper or run is flattened by the text write path, so its
//! block still takes the opaque path.

use super::BridgeError;
use crate::content::block::Block;
use crate::content::inline::Inline;
use crate::loro_schema::{BLOCK_TYPE_OPAQUE, KEY_OPAQUE_JSON, KEY_TYPE};
use loro::LoroMap;

/// Returns `true` when `block` survives the flat text-with-marks schema
/// without losing content, i.e. it does not need an opaque snapshot.
pub(super) fn block_round_trips_as_text(block: &Block) -> bool {
    match block {
        Block::Para(inlines) | Block::Plain(inlines) => inlines_round_trip(inlines),
        Block::StyledPara(para) => inlines_round_trip(&para.inlines),
        Block::Heading(_, _, inlines) => inlines_round_trip(inlines),
        Block::CodeBlock(_, _) | Block::HorizontalRule => true,
        _ => false,
    }
}

fn inlines_round_trip(inlines: &[Inline]) -> bool {
    inlines.iter().all(inline_round_trips_top)
}

/// Top-level inline (a direct child of the paragraph's inline list).
///
/// A bare image or footnote/endnote here is mapped natively by the write path —
/// an [`OBJECT_REPLACEMENT_CHAR`][crate::loro_schema::OBJECT_REPLACEMENT_CHAR]
/// anchor carrying the object as a mark — so the paragraph stays live-editable
/// instead of collapsing to an opaque snapshot. This requires `serde` (the
/// snapshot format); without it the object is not representable.
fn inline_round_trips_top(inline: &Inline) -> bool {
    match inline {
        Inline::Image(..) | Inline::Note(..) => cfg!(feature = "serde"),
        other => inline_round_trips_nested(other),
    }
}

/// Inline nested inside a wrapper or styled run. The write path flattens these
/// to plain text, so any structured object here (including a nested image)
/// cannot survive natively and forces the whole block to an opaque snapshot.
fn inline_round_trips_nested(inline: &Inline) -> bool {
    match inline {
        Inline::Str(_)
        | Inline::Space
        | Inline::SoftBreak
        | Inline::LineBreak
        | Inline::Code(_, _) => true,
        Inline::Emph(inner)
        | Inline::Strong(inner)
        | Inline::Underline(inner)
        | Inline::Strikeout(inner)
        | Inline::Superscript(inner)
        | Inline::Subscript(inner)
        | Inline::SmallCaps(inner)
        | Inline::Quoted(_, inner)
        | Inline::Span(_, inner)
        | Inline::Link(_, inner, _) => inner.iter().all(inline_round_trips_nested),
        Inline::StyledRun(run) => run.content.iter().all(inline_round_trips_nested),
        // Comment and bookmark anchors carry no body text and have always
        // been dropped by the text flattening; treating them as representable
        // keeps the vast majority of paragraphs editable.
        // TODO(loro-bridge): preserve comment/bookmark anchors as marks.
        Inline::Comment(_) | Inline::Bookmark(_, _) => true,
        // Nested Note/Image, Field, Math, RawInline, Cite — structured content
        // the flat text cannot carry (and, when nested, the native anchor path
        // does not reach it). The whole containing block is preserved as an
        // opaque snapshot instead.
        _ => false,
    }
}

/// Writes `block` into `map` as an opaque JSON snapshot.
#[cfg(feature = "serde")]
pub(super) fn write_opaque_block(block: &Block, map: &LoroMap) -> Result<(), BridgeError> {
    map.insert(KEY_TYPE, BLOCK_TYPE_OPAQUE)?;
    match serde_json::to_string(block) {
        Ok(json) => {
            map.insert(KEY_OPAQUE_JSON, json)?;
        }
        Err(err) => {
            // Unreachable in practice: every Block variant derives Serialize.
            tracing::warn!("loro bridge: failed to snapshot block: {err}");
        }
    }
    Ok(())
}

/// Legacy lossy fallback when the model is built without `serde`.
#[cfg(not(feature = "serde"))]
pub(super) fn write_opaque_block(_block: &Block, map: &LoroMap) -> Result<(), BridgeError> {
    tracing::warn!(
        "loro bridge: block dropped — build loki-doc-model with the `serde` \
         feature (default) to preserve unsupported blocks"
    );
    map.insert(KEY_TYPE, BLOCK_TYPE_OPAQUE)?;
    Ok(())
}

/// Reads an opaque snapshot back into the original [`Block`].
///
/// Falls back to [`Block::HorizontalRule`] when the snapshot is missing or
/// unparseable (e.g. written by a newer model version).
pub(super) fn read_opaque_block(map: &LoroMap) -> Block {
    let json = map
        .get(KEY_OPAQUE_JSON)
        .and_then(|v| v.into_value().ok())
        .and_then(|v| v.into_string().ok())
        .map(|s| s.to_string());
    let Some(json) = json else {
        return Block::HorizontalRule;
    };
    parse_snapshot(&json).unwrap_or_else(|| {
        tracing::warn!("loro bridge: unparseable opaque block snapshot");
        Block::HorizontalRule
    })
}

#[cfg(feature = "serde")]
fn parse_snapshot(json: &str) -> Option<Block> {
    serde_json::from_str(json).ok()
}

#[cfg(not(feature = "serde"))]
fn parse_snapshot(_json: &str) -> Option<Block> {
    None
}
