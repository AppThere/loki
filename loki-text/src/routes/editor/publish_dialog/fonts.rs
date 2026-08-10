// SPDX-License-Identifier: Apache-2.0

//! Which faces the document uses, and whether each can be embedded (design
//! note 29).
//!
//! # The one place bundled-vs-device has a licence consequence
//!
//! Everywhere else in Loki the distinction is about rendering. Here it decides
//! whether a face can be **put in the file**: a bundled face is licensed for
//! redistribution, a device face is licensed to the device. A reader without it
//! substitutes, and there is nothing the export can do about that — so the
//! table says so per face rather than in a footnote.

use std::collections::BTreeMap;

use loki_doc_model::Document;
use loki_doc_model::content::block::Block;
use loki_doc_model::content::inline::Inline;
use loki_fonts::is_bundled_family;

use super::super::dialog_walk::{block_inlines, visit_doc_blocks, visit_inlines};

/// One face the document asks for.
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub(super) struct UsedFace {
    /// The family name as the document names it.
    pub name: String,
    /// Whether it ships with Loki and can therefore be embedded.
    pub bundled: bool,
}

/// Every family the document references, sorted, deduplicated.
///
/// Collected from **style definitions and direct runs together**: a face used
/// only by a character style still ends up in the file, and a table that listed
/// direct formatting alone would under-report what the export must carry.
#[must_use]
pub(super) fn used_faces(doc: &Document) -> Vec<UsedFace> {
    let mut names: BTreeMap<String, String> = BTreeMap::new();

    for style in doc.styles.paragraph_styles.values() {
        if let Some(name) = style.char_props.font_name.as_ref() {
            insert_name(&mut names, name);
        }
    }
    for style in doc.styles.character_styles.values() {
        if let Some(name) = style.char_props.font_name.as_ref() {
            insert_name(&mut names, name);
        }
    }
    // One shared walk (`dialog_walk`), so table head/foot cells and styled
    // paragraphs are covered here the same way they are everywhere else.
    visit_doc_blocks(doc, &mut |block| {
        if let Block::StyledPara(para) = block
            && let Some(props) = para.direct_char_props.as_ref()
            && let Some(name) = props.font_name.as_ref()
        {
            insert_name(&mut names, name);
        }
        for inlines in block_inlines(block) {
            visit_inlines(inlines, &mut |inline| {
                if let Inline::StyledRun(run) = inline
                    && let Some(props) = run.direct_props.as_ref()
                    && let Some(name) = props.font_name.as_ref()
                {
                    insert_name(&mut names, name);
                }
            });
        }
    });

    names
        .into_values()
        .map(|name| UsedFace {
            bundled: is_bundled_family(&name),
            name,
        })
        .collect()
}

/// Adds a family name, ignoring blanks.
///
/// Keyed on the **lowercased** name, because family names arrive as free text
/// from ODF and OOXML and `is_bundled_family` already treats `TINOS` and
/// `tinos` as one face. Keying on the raw name listed a case-varied import
/// twice and inflated the "will be substituted" count with it. The first
/// spelling seen is the one displayed.
fn insert_name(names: &mut BTreeMap<String, String>, name: &str) {
    let trimmed = name.trim();
    if !trimmed.is_empty() {
        names
            .entry(trimmed.to_lowercase())
            .or_insert_with(|| trimmed.to_string());
    }
}

/// How many of the used faces can be embedded.
#[must_use]
pub(super) fn embeddable_count(faces: &[UsedFace]) -> usize {
    faces.iter().filter(|f| f.bundled).count()
}

#[cfg(test)]
#[path = "fonts_tests.rs"]
mod tests;
