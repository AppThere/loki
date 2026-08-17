// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 AppThere Loki contributors

//! Collector for ODF *automatic styles* — the per-use styles emitted into
//! `content.xml`'s `<office:automatic-styles>` for direct (inline / paragraph)
//! formatting that is not a named catalog style.

use std::collections::HashMap;

use loki_doc_model::style::props::char_props::CharProps;
use loki_doc_model::style::props::para_props::ParaProps;
use loki_doc_model::style::table_borders::CellEdges;
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
    /// graphic-properties element → style name (`gr{n}`).
    graphic: HashMap<String, String>,
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
    /// **effective** `background`, `edges` and `padding`, or `None` when there
    /// is none of any. All three are resolved values (direct formatting else
    /// the table style's contribution) because ODF has no conditional-region,
    /// table-level-border or table-level-cell-margin concept — it bakes all
    /// three into per-cell styles, so resolution happens before serialisation.
    ///
    /// The cell's own `CellProps` is deliberately *not* a parameter. Reading
    /// `props.border_*` / `props.padding_*` here — instead of the resolved
    /// values — is exactly the bug this signature replaces, twice over; with
    /// the raw props out of scope there is no path through this function that
    /// can see them.
    pub(super) fn cell_style(
        &mut self,
        background: Option<&DocumentColor>,
        edges: &CellEdges,
        padding: &super::tables::EffectivePadding,
    ) -> Option<String> {
        let cell_props = emit_cell_properties(background, edges, padding);
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

    /// Returns the automatic `family="graphic"` style name for a floating
    /// frame's `style:graphic-properties` (`wrap` / `run-through`, solid fill,
    /// solid stroke), or `None` when none of them is set. Used by the text-box
    /// writer so a `draw:frame`'s wrap + fill + border round-trip.
    pub(super) fn graphic_style(
        &mut self,
        wrap: Option<loki_doc_model::content::float::FloatWrap>,
        fill: Option<&str>,
        stroke: Option<&str>,
    ) -> Option<String> {
        let props = graphic::emit_graphic_properties(wrap, fill, stroke);
        if props.is_empty() {
            return None;
        }
        if let Some(name) = self.graphic.get(&props) {
            return Some(name.clone());
        }
        let name = format!("gr{}", self.rendered.len() + 1);
        self.rendered.push(format!(
            "<style:style style:name=\"{name}\" style:family=\"graphic\">{props}</style:style>"
        ));
        self.graphic.insert(props, name.clone());
        Some(name)
    }

    /// Renders all collected automatic styles as concatenated XML.
    pub(super) fn render(&self) -> String {
        self.rendered.concat()
    }
}

#[path = "auto_graphic.rs"]
mod graphic;

/// Serialises a cell's exportable direct properties — background, borders,
/// and padding (4a.3) — as a `<style:table-cell-properties/>` element, or an
/// empty string when the cell carries none.
fn emit_cell_properties(
    background: Option<&DocumentColor>,
    edges: &CellEdges,
    padding: &super::tables::EffectivePadding,
) -> String {
    let mut s = String::new();
    if let Some(hex) = background.and_then(DocumentColor::to_hex) {
        attr_str(&mut s, "fo:background-color", &hex);
    }
    // `edges` is (top, right, bottom, left) — already resolved against the
    // table style by the caller.
    super::para_props::border_attr(&mut s, "fo:border-top", edges.0.as_ref());
    super::para_props::border_attr(&mut s, "fo:border-bottom", edges.2.as_ref());
    super::para_props::border_attr(&mut s, "fo:border-left", edges.3.as_ref());
    super::para_props::border_attr(&mut s, "fo:border-right", edges.1.as_ref());
    // `padding` is passed in, already resolved against the table style, for the
    // same reason `edges` is: reading `props.padding_*` here would silently drop
    // every inherited `w:tblCellMar`.
    for (name, pad) in [
        ("fo:padding-top", padding.0),
        ("fo:padding-bottom", padding.1),
        ("fo:padding-left", padding.2),
        ("fo:padding-right", padding.3),
    ] {
        if let Some(p) = pad {
            attr_str(&mut s, name, &format!("{}pt", p.value()));
        }
    }
    if s.is_empty() {
        return String::new();
    }
    format!("<style:table-cell-properties{s}/>")
}

/// Appends ` name="value"` (value pre-escaped / numeric).
fn attr_str(s: &mut String, name: &str, value: &str) {
    s.push(' ');
    s.push_str(name);
    s.push_str("=\"");
    s.push_str(value);
    s.push('"');
}

#[cfg(test)]
#[path = "auto_tests.rs"]
mod tests;
