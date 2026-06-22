// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 AppThere Loki contributors

//! Collector for ODF *automatic styles* — the per-use styles emitted into
//! `content.xml`'s `<office:automatic-styles>` for direct (inline / paragraph)
//! formatting that is not a named catalog style.

use std::collections::HashMap;

use loki_doc_model::style::props::char_props::CharProps;
use loki_doc_model::style::props::para_props::ParaProps;

use super::para_props::emit_paragraph_properties;
use super::props::emit_text_properties;

/// Deduplicates automatic text (`family="text"`) and paragraph
/// (`family="paragraph"`) styles, assigning stable `T{n}` / `P{n}` names.
#[derive(Default)]
pub(super) struct AutoStyles {
    /// text-properties element → style name (`T{n}`).
    text: HashMap<String, String>,
    /// (parent, paragraph-properties, text-properties) → style name (`P{n}`).
    para: HashMap<(String, String, String), String>,
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
        let key = (
            parent.unwrap_or("").to_string(),
            p_props.clone(),
            t_props.clone(),
        );
        if let Some(name) = self.para.get(&key) {
            return Some(name.clone());
        }
        let name = format!("P{}", self.rendered.len() + 1);
        let mut el = format!("<style:style style:name=\"{name}\" style:family=\"paragraph\"");
        if let Some(parent) = parent {
            el.push_str(&format!(" style:parent-style-name=\"{parent}\""));
        }
        el.push('>');
        el.push_str(&p_props);
        el.push_str(&t_props);
        el.push_str("</style:style>");
        self.rendered.push(el);
        self.para.insert(key, name.clone());
        Some(name)
    }

    /// Renders all collected automatic styles as concatenated XML.
    pub(super) fn render(&self) -> String {
        self.rendered.concat()
    }
}
