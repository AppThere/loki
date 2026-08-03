// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 AppThere Loki contributors

//! The **advisory page-style part** for DOCX (Spec 08 T6.5, decision D-02).
//!
//! # What DOCX cannot say
//!
//! OOXML has no named page style. A section's geometry lives in its `w:sectPr`
//! and nothing else; the names Loki assigns on import — or that a user typed in
//! the style panel — have nowhere to go, so a rename survives ODT export (which
//! has `style:master-page`) and is lost on DOCX export. This part carries the
//! names, and the section → name mapping, beside the document rather than in it.
//!
//! # Advisory means four things, and each is enforced rather than asserted
//!
//! * **Ignored by Word.** It is a private part with a private content type and
//!   a private relationship type. Nothing in `document.xml` refers to it, so a
//!   consumer that does not know it exists reads exactly the document it would
//!   have read without it.
//! * **Never affects geometry.** [`apply_page_style_part`] writes
//!   `Section::page_style` and catalog display names, and touches no
//!   `PageLayout` field. `the_part_never_changes_geometry` asserts that by
//!   comparing every section's layout before and after.
//! * **Validated against the `sectPr` count.** The part records how many
//!   sections it described. A document whose section count has changed since —
//!   because Word split or merged one, or because a different producer rewrote
//!   it — gets no names rather than the wrong ones.
//! * **Discarded on mismatch.** Any inconsistency drops the *whole* part, not
//!   the offending entry: a partial mapping would leave some sections named and
//!   others not, which is harder to explain than no names at all.
//!
//! # Why not `customXml/`
//!
//! That is a public OPC affordance with its own item/itemProps pair and a
//! datastore Word surfaces in its UI. This is not user data — it is a private
//! side-channel for something the format cannot express — so it takes a private
//! part name and stays out of a place users can see and edit.

use loki_doc_model::document::Document;
use loki_doc_model::style::catalog::StyleId;
use loki_doc_model::style::page_style::PageStyle;

use crate::xml_util::escape_xml;

/// Part name for the advisory page-style map.
pub(crate) const PAGE_STYLE_PART: &str = "/word/lokiPageStyles.xml";

/// Content type for the advisory part. Private to Loki — a consumer that does
/// not recognise it has no reason to open the part.
pub(crate) const MT_PAGE_STYLES: &str = "application/vnd.appthere.loki.pagestyles+xml";

/// Relationship type from the main document part to the advisory part.
pub(crate) const REL_PAGE_STYLES: &str = "http://appthere.dev/loki/2026/relationships/pageStyles";

/// XML namespace for the part's own root element.
const NS_PAGE_STYLES: &str = "http://appthere.dev/loki/2026/pageStyles";

/// One page style as the part records it.
#[derive(Debug, Clone, PartialEq)]
pub(crate) struct PartStyle {
    /// The style id — what `Section::page_style` references.
    pub id: String,
    /// The human-readable name, when it differs from the id.
    pub display_name: Option<String>,
}

/// The part's contents: the styles, and one style id per section.
#[derive(Debug, Clone, PartialEq, Default)]
pub(crate) struct PageStyleMap {
    /// Declared styles, in catalog order.
    pub styles: Vec<PartStyle>,
    /// Each section's style id, index-aligned with the document's sections.
    /// `None` for a section that had no page style.
    pub sections: Vec<Option<String>>,
}

impl PageStyleMap {
    /// Reads the map a `doc` would export. `None` when no section names a page
    /// style — there is nothing to advise, and an empty part is a part a reader
    /// has to consider.
    pub(crate) fn from_document(doc: &Document) -> Option<Self> {
        let sections: Vec<Option<String>> = doc
            .sections
            .iter()
            .map(|s| s.page_style.as_ref().map(|id| id.as_str().to_string()))
            .collect();
        if sections.iter().all(Option::is_none) {
            return None;
        }
        let styles = doc
            .styles
            .page_styles
            .iter()
            .map(|(id, ps)| PartStyle {
                id: id.as_str().to_string(),
                display_name: ps.display_name.clone().filter(|n| n != id.as_str()),
            })
            .collect();
        Some(Self { styles, sections })
    }

    /// Serialises the part.
    ///
    /// `sectionCount` is written explicitly rather than left implicit in the
    /// number of `<section>` elements: it is the value a reader checks against
    /// the document it actually parsed, and a count derived from the same list
    /// it is meant to validate would agree with itself no matter what.
    pub(crate) fn to_xml(&self) -> String {
        let mut out = String::from(r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?>"#);
        out.push_str(&format!(
            r#"<pageStyles xmlns="{NS_PAGE_STYLES}" sectionCount="{}">"#,
            self.sections.len()
        ));
        for style in &self.styles {
            out.push_str(&format!(r#"<style id="{}""#, escape_xml(&style.id)));
            if let Some(name) = &style.display_name {
                out.push_str(&format!(r#" displayName="{}""#, escape_xml(name)));
            }
            out.push_str("/>");
        }
        for (i, section) in self.sections.iter().enumerate() {
            if let Some(id) = section {
                out.push_str(&format!(
                    r#"<section index="{i}" style="{}"/>"#,
                    escape_xml(id)
                ));
            }
        }
        out.push_str("</pageStyles>");
        out
    }
}

/// Applies a parsed `map` to `doc` — names only.
///
/// Returns `false` and changes nothing when the map does not describe this
/// document: a differing section count, an out-of-range index, or a section
/// naming a style the part did not declare. See the module docs on why the
/// whole part is dropped rather than the offending entry.
pub(crate) fn apply_page_style_part(doc: &mut Document, map: &PageStyleMap) -> bool {
    if map.sections.len() != doc.sections.len() {
        return false;
    }
    let declared: Vec<&str> = map.styles.iter().map(|s| s.id.as_str()).collect();
    if map
        .sections
        .iter()
        .flatten()
        .any(|id| !declared.contains(&id.as_str()))
    {
        return false;
    }

    for (section, id) in doc.sections.iter_mut().zip(map.sections.iter()) {
        // Only the reference. The section's `layout` is whatever `w:sectPr`
        // said, and stays that way — this part is advisory about *names*.
        section.page_style = id.as_ref().map(|s| StyleId::new(s.as_str()));
    }
    for style in &map.styles {
        let id = StyleId::new(style.id.as_str());
        let layout = doc
            .sections
            .iter()
            .find(|s| s.page_style.as_ref() == Some(&id))
            .map(|s| s.layout.clone())
            .unwrap_or_default();
        let entry = doc
            .styles
            .page_styles
            .entry(id.clone())
            .or_insert_with(|| PageStyle::new(id, layout));
        entry.display_name.clone_from(&style.display_name);
    }
    true
}

#[cfg(test)]
#[path = "page_style_part_tests.rs"]
mod tests;
