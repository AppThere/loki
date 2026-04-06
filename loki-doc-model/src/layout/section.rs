// Copyright 2024-2026 AppThere
//
// Licensed under the Apache License, Version 2.0 (the "License");
// you may not use this file except in compliance with the License.
// You may obtain a copy of the License at
//
//     http://www.apache.org/licenses/LICENSE-2.0
//
// Unless required by applicable law or agreed to in writing, software
// distributed under the License is distributed on an "AS IS" BASIS,
// WITHOUT WARRANTIES OR CONDITIONS OF ANY KIND, either express or implied.
// See the License for the specific language governing permissions and
// limitations under the License.

//! Document section type.
//!
//! A [`Section`] is a contiguous sequence of blocks that share a common
//! page layout. Most simple documents have exactly one section.
//!
//! TR 29166 §7.2.8 (Section and page layout).
//! ODF: implicit via `<style:master-page>` page break transitions.
//! OOXML: `<w:sectPr>` element at the end of the section.

use crate::content::attr::ExtensionBag;
use crate::content::block::Block;
use crate::layout::page::PageLayout;

/// A document section — a contiguous sequence of blocks sharing a
/// common page layout.
///
/// ODF: implicit section via `<style:master-page>` page break.
/// OOXML: `<w:sectPr>` at the end of the section.
/// TR 29166 §7.2.8 (Section and page layout).
///
/// If the document has no explicit section breaks, there is exactly one
/// [`Section`] containing all content. See [`crate::Document`].
#[derive(Debug, Clone, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct Section {
    /// The page layout applied to this section.
    pub layout: PageLayout,
    /// The block-level content of this section.
    pub blocks: Vec<Block>,
    /// Format-specific extension data.
    pub extensions: ExtensionBag,
}

impl Section {
    /// Creates a new section with the default page layout and no content.
    #[must_use]
    pub fn new() -> Self {
        Self {
            layout: PageLayout::default(),
            blocks: Vec::new(),
            extensions: ExtensionBag::default(),
        }
    }

    /// Creates a section with the given page layout and content.
    #[must_use]
    pub fn with_layout_and_blocks(layout: PageLayout, blocks: Vec<Block>) -> Self {
        Self {
            layout,
            blocks,
            extensions: ExtensionBag::default(),
        }
    }
}

impl Default for Section {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::content::block::Block;
    use crate::content::attr::NodeAttr;
    use crate::content::inline::Inline;
    use crate::layout::page::{PageLayout, PageSize};

    #[test]
    fn section_default_is_empty() {
        let s = Section::new();
        assert!(s.blocks.is_empty());
    }

    #[test]
    fn section_with_content() {
        let layout = PageLayout::default();
        let blocks = vec![
            Block::Heading(1, NodeAttr::default(), vec![Inline::Str("Title".into())]),
        ];
        let section = Section::with_layout_and_blocks(layout.clone(), blocks);
        assert_eq!(section.blocks.len(), 1);
    }

    #[test]
    fn two_sections_with_different_page_sizes() {
        let mut layout_a = PageLayout::default();
        layout_a.page_size = PageSize::a4();
        let mut layout_b = PageLayout::default();
        layout_b.page_size = PageSize::letter();

        let s1 = Section::with_layout_and_blocks(layout_a, vec![]);
        let s2 = Section::with_layout_and_blocks(layout_b, vec![]);
        assert_ne!(s1.layout.page_size, s2.layout.page_size);
    }
}
