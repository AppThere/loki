// Copyright 2026 AppThere Loki contributors
// SPDX-License-Identifier: Apache-2.0

//! Header and footer types.
//!
//! TR 29166 §6.2.3 "Header and footer" describes the ODF and OOXML header
//! and footer models. ODF: `style:header` / `style:footer` inside a master
//! page. OOXML: `w:headerReference` / `w:footerReference` inside `w:sectPr`.

use crate::content::block::Block;

/// Identifies which pages a header or footer applies to.
///
/// TR 29166 §6.2.3. Both ODF and OOXML support separate first-page and
/// odd/even headers. ODF uses separate `style:header-first` and
/// `style:header-left` elements; OOXML uses `w:headerReference` with
/// `w:type` = `"first"`, `"even"`, `"default"`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[non_exhaustive]
pub enum HeaderFooterKind {
    /// The default (odd-page) header or footer.
    Default,
    /// The first-page header or footer.
    First,
    /// The even-page header or footer.
    Even,
}

/// A header or footer region containing block content.
///
/// TR 29166 §6.2.3 "Header and footer" feature table.
///
/// ODF: `style:header` / `style:footer` (and `-first`, `-left` variants)
/// inside `style:master-page`.
/// OOXML: `w:hdr` / `w:ftr` document parts referenced from `w:sectPr`.
#[derive(Debug, Clone, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct HeaderFooter {
    /// Whether this is a header or footer region.
    pub kind: HeaderFooterKind,
    /// The content of the header or footer as a sequence of blocks.
    pub blocks: Vec<Block>,
}

impl HeaderFooter {
    /// Creates a new empty [`HeaderFooter`] of the given kind.
    #[must_use]
    pub fn new(kind: HeaderFooterKind) -> Self {
        Self {
            kind,
            blocks: Vec::new(),
        }
    }

    /// Creates a default-kind header or footer with the given content.
    #[must_use]
    pub fn default_with_blocks(blocks: Vec<Block>) -> Self {
        Self {
            kind: HeaderFooterKind::Default,
            blocks,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn empty_header() {
        let h = HeaderFooter::new(HeaderFooterKind::Default);
        assert_eq!(h.kind, HeaderFooterKind::Default);
        assert!(h.blocks.is_empty());
    }

    #[test]
    fn first_page_footer() {
        let f = HeaderFooter::new(HeaderFooterKind::First);
        assert_eq!(f.kind, HeaderFooterKind::First);
    }
}
