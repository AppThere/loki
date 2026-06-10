// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 AppThere Loki contributors

//! Opaque-snapshot preservation for blocks the Loro schema cannot represent.
//!
//! The CRDT schema flattens paragraph content into a [`loro::LoroText`] with
//! marks, which cannot carry structured content (tables, lists, figures,
//! footnote bodies, inline images, fields, math). Before this module existed,
//! such blocks were written as empty stubs and read back as
//! `Block::HorizontalRule` — i.e. a single edit cycle destroyed them.
//!
//! Instead, any block that cannot round-trip through the text schema is
//! serialized to JSON (via the model's `serde` derives) and stored verbatim
//! under [`KEY_OPAQUE_JSON`]. On read it is deserialized back into the exact
//! original [`Block`]. Opaque blocks render normally but are not
//! collaboratively editable from within; converting them to native CRDT
//! mappings is tracked as TODO(loro-bridge) work.

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
    inlines.iter().all(|inline| match inline {
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
        | Inline::Link(_, inner, _) => inlines_round_trip(inner),
        Inline::StyledRun(run) => inlines_round_trip(&run.content),
        // Comment and bookmark anchors carry no body text and have always
        // been dropped by the text flattening; treating them as representable
        // keeps the vast majority of paragraphs editable.
        // TODO(loro-bridge): preserve comment/bookmark anchors as marks.
        Inline::Comment(_) | Inline::Bookmark(_, _) => true,
        // Note (footnote/endnote bodies), Image, Field, Math, RawInline,
        // Cite — structured content the flat text cannot carry. The whole
        // containing block is preserved as an opaque snapshot instead.
        _ => false,
    })
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
