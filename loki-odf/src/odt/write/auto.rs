// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 AppThere Loki contributors

//! Collector for ODF *automatic styles* — the per-use styles emitted into
//! `content.xml`'s `<office:automatic-styles>` for direct (inline / paragraph)
//! formatting that is not a named catalog style.

use std::collections::HashMap;

use loki_doc_model::style::props::char_props::CharProps;
use loki_doc_model::style::props::para_props::ParaProps;
use loki_primitives::color::DocumentColor;

use super::para_props::emit_paragraph_properties;
use super::props::emit_text_properties;

/// Deduplicates automatic text (`family="text"`), paragraph
/// (`family="paragraph"`), and table-cell (`family="table-cell"`) styles,
/// assigning stable `T{n}` / `P{n}` / `TC{n}` names.
#[derive(Default)]
pub(super) struct AutoStyles {
    /// text-properties element → style name (`T{n}`).
    text: HashMap<String, String>,
    /// (parent, paragraph-properties, text-properties) → style name (`P{n}`).
    para: HashMap<(String, String, String), String>,
    /// table-cell-properties element → style name (`TC{n}`).
    cell: HashMap<String, String>,
    /// Rendered `<style:style>` elements, in creation order.
    rendered: Vec<String>,
}

impl AutoStyles {
    pub(super) fn new() -> Self {
        Self::default()
    }

    /// Returns the automatic text-style name for `cp`, or `None` when `cp`
    /// carries no formatting.
    pub(super) fn text_style(&mut self, cp: &CharProps) -> Option<String> {
        let props = emit_text_properties(cp);
        if props.is_empty() {
            return None;
        }
        if let Some(name) = self.text.get(&props) {
            return Some(name.clone());
        }
        let name = format!("T{}", self.rendered.len() + 1);
        self.rendered.push(format!(
            "<style:style style:name=\"{name}\" style:family=\"text\">{props}</style:style>"
        ));
        self.text.insert(props, name.clone());
        Some(name)
    }

    /// Returns the automatic paragraph-style name for direct paragraph/char
    /// formatting with optional `parent`, or `None` when nothing is set.
    pub(super) fn para_style(
        &mut self,
        parent: Option<&str>,
        pp: &ParaProps,
        cp: &CharProps,
    ) -> Option<String> {
        let p_props = emit_paragraph_properties(pp);
        let t_props = emit_text_properties(cp);
        if p_props.is_empty() && t_props.is_empty() && parent.is_none() {
            return None;
        }
        Some(self.para_style_inner(parent, &p_props, &t_props, None))
    }

    /// Like [`Self::para_style`] but forces an automatic style carrying
    /// `style:master-page-name` — used on the first paragraph of each section
    /// to trigger the master-page (page-geometry) transition on re-import.
    ///
    /// Always returns a name, since the master-page reference must be emitted
    /// even when the paragraph has no other direct formatting.
    pub(super) fn para_style_master(
        &mut self,
        parent: Option<&str>,
        pp: &ParaProps,
        cp: &CharProps,
        master_page: &str,
    ) -> String {
        let p_props = emit_paragraph_properties(pp);
        let t_props = emit_text_properties(cp);
        self.para_style_inner(parent, &p_props, &t_props, Some(master_page))
    }

    /// Builds (and deduplicates) a `family="paragraph"` automatic style from
    /// pre-rendered property strings, with an optional master-page reference.
    fn para_style_inner(
        &mut self,
        parent: Option<&str>,
        p_props: &str,
        t_props: &str,
        master_page: Option<&str>,
    ) -> String {
        let key = (
            format!("{}\u{1}{}", parent.unwrap_or(""), master_page.unwrap_or("")),
            p_props.to_string(),
            t_props.to_string(),
        );
        if let Some(name) = self.para.get(&key) {
            return name.clone();
        }
        let name = format!("P{}", self.rendered.len() + 1);
        let mut el = format!("<style:style style:name=\"{name}\" style:family=\"paragraph\"");
        if let Some(parent) = parent {
            el.push_str(&format!(" style:parent-style-name=\"{parent}\""));
        }
        if let Some(mp) = master_page {
            el.push_str(&format!(" style:master-page-name=\"{mp}\""));
        }
        el.push('>');
        el.push_str(p_props);
        el.push_str(t_props);
        el.push_str("</style:style>");
        self.rendered.push(el);
        self.para.insert(key, name.clone());
        name
    }

    /// Returns the automatic `family="table-cell"` style name for a cell's
    /// effective `background` (its direct shading or the resolved table-style
    /// banding), or `None` when there is none. `fo:background-color` is the
    /// ODF-native representation of table-style banding, since ODF bakes region
    /// shading into per-cell styles rather than conditional regions. (Cell
    /// borders/padding export is future work.)
    pub(super) fn cell_style(&mut self, background: Option<&DocumentColor>) -> Option<String> {
        let cell_props = emit_cell_properties(background);
        if cell_props.is_empty() {
            return None;
        }
        if let Some(name) = self.cell.get(&cell_props) {
            return Some(name.clone());
        }
        let name = format!("TC{}", self.rendered.len() + 1);
        self.rendered.push(format!(
            "<style:style style:name=\"{name}\" style:family=\"table-cell\">{cell_props}</style:style>"
        ));
        self.cell.insert(cell_props, name.clone());
        Some(name)
    }

    /// Renders all collected automatic styles as concatenated XML.
    pub(super) fn render(&self) -> String {
        self.rendered.concat()
    }
}

/// Serialises a cell's exportable direct properties as a
/// `<style:table-cell-properties/>` element, or an empty string when none.
fn emit_cell_properties(background: Option<&DocumentColor>) -> String {
    let Some(hex) = background.and_then(DocumentColor::to_hex) else {
        return String::new();
    };
    format!("<style:table-cell-properties fo:background-color=\"{hex}\"/>")
}

#[cfg(test)]
mod tests {
    use super::*;
    use loki_primitives::color::RgbColor;

    fn color(r: u8, g: u8, b: u8) -> DocumentColor {
        DocumentColor::Rgb(RgbColor::new(
            f32::from(r) / 255.0,
            f32::from(g) / 255.0,
            f32::from(b) / 255.0,
        ))
    }

    #[test]
    fn cell_style_emits_background_and_dedupes() {
        let mut a = AutoStyles::new();
        let n1 = a
            .cell_style(Some(&color(0x44, 0x72, 0xC4)))
            .expect("shaded cell → style");
        // Same colour reuses the same style name.
        let n2 = a.cell_style(Some(&color(0x44, 0x72, 0xC4))).unwrap();
        assert_eq!(n1, n2);
        // A different colour gets a distinct name.
        let n3 = a.cell_style(Some(&color(0xFF, 0x00, 0x00))).unwrap();
        assert_ne!(n1, n3);

        let xml = a.render();
        assert!(xml.contains(r#"style:family="table-cell""#));
        assert!(xml.contains(r##"fo:background-color="#4472C4""##));
        assert!(xml.contains(r##"fo:background-color="#FF0000""##));
    }

    #[test]
    fn a_cell_without_shading_gets_no_style() {
        let mut a = AutoStyles::new();
        assert_eq!(a.cell_style(None), None);
        assert!(a.render().is_empty());
    }
}
