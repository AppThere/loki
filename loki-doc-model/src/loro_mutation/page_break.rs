// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 AppThere Loki contributors

//! The page-break-before flag on a block's direct paragraph props — the
//! model's expression of a hard page break (the Insert tab splits the
//! paragraph at the caret and flags the following block).
//!
//! Storage mirrors [`super::align`]: a plain `para` has no props slot on
//! read, so it is upgraded to `styled_para` before the flag lands in its
//! `para_props` sub-map.

use loro::{LoroDoc, LoroMap};

use super::{MutationError, get_block_map_and_list};
use crate::loro_schema::{
    BLOCK_TYPE_PARA, BLOCK_TYPE_STYLED_PARA, KEY_PARA_PROPS, KEY_TYPE, PROP_PAGE_BREAK_BEFORE,
};

fn block_type(block_map: &LoroMap) -> String {
    block_map
        .get(KEY_TYPE)
        .and_then(|v| v.into_value().ok())
        .and_then(|v| v.into_string().ok())
        .map(|s| s.to_string())
        .unwrap_or_default()
}

/// Sets (or clears) `page_break_before` on the top-level block at
/// `block_index`. A plain paragraph is upgraded to a styled paragraph so the
/// flag persists — the same trick alignment uses.
///
/// # Errors
///
/// - [`MutationError::BlockIndexOutOfRange`] if `block_index` is out of range.
/// - [`MutationError::Loro`] for underlying Loro errors.
pub fn set_block_page_break_before(
    loro: &LoroDoc,
    block_index: usize,
    on: bool,
) -> Result<(), MutationError> {
    let (_, block_map, _) = get_block_map_and_list(loro, block_index)?;
    if matches!(block_type(&block_map).as_str(), BLOCK_TYPE_PARA | "") {
        block_map.insert(KEY_TYPE, BLOCK_TYPE_STYLED_PARA)?;
    }
    let props = if let Some(existing) = block_map
        .get(KEY_PARA_PROPS)
        .and_then(|v| v.into_container().ok())
        .and_then(|c| c.into_map().ok())
    {
        existing
    } else {
        block_map.insert_container(KEY_PARA_PROPS, LoroMap::new())?
    };
    props.insert(PROP_PAGE_BREAK_BEFORE, on)?;
    Ok(())
}

/// Whether the top-level block at `block_index` carries `page_break_before`.
#[must_use]
pub fn get_block_page_break_before(loro: &LoroDoc, block_index: usize) -> bool {
    get_block_map_and_list(loro, block_index)
        .ok()
        .and_then(|(_, m, _)| {
            m.get(KEY_PARA_PROPS)
                .and_then(|v| v.into_container().ok())
                .and_then(|c| c.into_map().ok())
        })
        .and_then(|props| props.get(PROP_PAGE_BREAK_BEFORE))
        .and_then(|v| v.into_value().ok())
        .and_then(|v| v.into_bool().ok())
        .unwrap_or(false)
}

#[cfg(test)]
mod tests {
    use loro::LoroDoc;

    use super::{get_block_page_break_before, set_block_page_break_before};
    use crate::content::block::Block;
    use crate::content::inline::Inline;
    use crate::document::Document;
    use crate::loro_bridge::{document_to_loro, loro_to_document};

    fn loro_with_two_paras() -> LoroDoc {
        let mut doc = Document::new();
        doc.sections[0].blocks = vec![
            Block::Para(vec![Inline::Str("one".into())]),
            Block::Para(vec![Inline::Str("two".into())]),
        ];
        document_to_loro(&doc).expect("to loro")
    }

    #[test]
    fn flag_lands_survives_derivation_and_clears() {
        let loro = loro_with_two_paras();
        set_block_page_break_before(&loro, 1, true).expect("set");
        assert!(get_block_page_break_before(&loro, 1));
        assert!(
            !get_block_page_break_before(&loro, 0),
            "neighbour untouched"
        );

        // The derived document sees the flag where the layout reads it.
        let doc = loro_to_document(&loro).expect("derive");
        let Block::StyledPara(sp) = &doc.sections[0].blocks[1] else {
            panic!("plain para must upgrade to styled");
        };
        assert_eq!(
            sp.direct_para_props
                .as_ref()
                .and_then(|p| p.page_break_before),
            Some(true)
        );

        set_block_page_break_before(&loro, 1, false).expect("clear");
        assert!(!get_block_page_break_before(&loro, 1));
    }

    #[test]
    fn out_of_range_is_a_typed_refusal() {
        let loro = loro_with_two_paras();
        assert!(set_block_page_break_before(&loro, 9, true).is_err());
    }
}
