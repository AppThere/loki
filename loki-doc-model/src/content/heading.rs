// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 AppThere Loki contributors

//! The style a [`Block::Heading`](crate::content::block::Block::Heading)
//! resolves through.
//!
//! Unlike [`StyledParagraph`](crate::content::block::StyledParagraph), which
//! names its style in a typed `style_id` field, `Block::Heading` carries the
//! name in its [`NodeAttr`] under the `"style"` key — set by the ODF mapper
//! from `text:style-name` and by the OOXML mapper from `w:pStyle` during
//! heading promotion. Nothing in the type makes that lookup discoverable, so
//! each consumer restated it locally and the ODT writer simply never did:
//! it synthesised `Heading{level}` from the level and dropped an imported
//! `Heading_20_1`, leaving the body referencing a style the written
//! `styles.xml` does not declare.
//!
//! [`heading_style_id`] is the one derivation. Resolve a heading's style
//! through it rather than reading the attr directly, so a consumer cannot
//! answer this question differently by omitting the lookup.

use crate::content::attr::NodeAttr;
use crate::style::catalog::StyleId;

/// The `NodeAttr` key under which a promoted heading carries its style name.
pub const HEADING_STYLE_KEY: &str = "style";

/// The paragraph-style id a heading resolves through.
///
/// Prefers the name carried in `attr` (an imported document's own heading
/// style, whatever it is called — `Heading_20_1`, `SceneHeading`, …). Falls
/// back to the canonical `Heading{level}` name for a heading that carries
/// none, which is the case for one created in-app rather than imported.
///
/// The fallback clamps to the six levels that exist as named styles: a
/// `Heading7` reference would dangle exactly the way the bug this function
/// replaces did.
pub fn heading_style_id(level: u8, attr: &NodeAttr) -> StyleId {
    attr.kv
        .iter()
        .find(|(k, _)| k == HEADING_STYLE_KEY)
        .map_or_else(
            || StyleId::new(format!("Heading{}", level.clamp(1, 6))),
            |(_, v)| StyleId::new(v.as_str()),
        )
}

#[cfg(test)]
#[path = "heading_tests.rs"]
mod tests;
