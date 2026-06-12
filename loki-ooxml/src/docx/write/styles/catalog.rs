// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 AppThere Loki contributors

//! Named paragraph and character style serializer from the [`StyleCatalog`].

use quick_xml::Writer;

use loki_doc_model::style::catalog::StyleCatalog;

use crate::docx::write::xml::{write_empty, write_end, write_start, wval};

use super::headings::HEADINGS;
use super::props::{write_char_props_elem, write_para_props_elem};

pub(super) fn write_catalog_styles<W: std::io::Write>(w: &mut Writer<W>, catalog: &StyleCatalog) {
    for (id, style) in &catalog.paragraph_styles {
        let sid = id.as_str();
        // Skip Normal and Heading1-6 — already emitted above.
        if sid == "Normal" || HEADINGS.iter().any(|(h, _, _, _, _)| *h == sid) {
            continue;
        }
        let is_default_s = if style.is_default { "1" } else { "0" };
        let _ = write_start(
            w,
            "w:style",
            &[
                ("w:type", "paragraph"),
                ("w:default", is_default_s),
                ("w:styleId", sid),
            ],
        );
        let name = style.display_name.as_deref().unwrap_or(sid);
        let _ = write_empty(w, "w:name", &wval(name));
        if let Some(ref parent) = style.parent {
            let _ = write_empty(w, "w:basedOn", &wval(parent.as_str()));
        }
        write_para_props_elem(w, &style.para_props);
        write_char_props_elem(w, &style.char_props);
        let _ = write_end(w, "w:style");
    }

    for (id, style) in &catalog.character_styles {
        let sid = id.as_str();
        let _ = write_start(w, "w:style", &[("w:type", "character"), ("w:styleId", sid)]);
        let name = style.display_name.as_deref().unwrap_or(sid);
        let _ = write_empty(w, "w:name", &wval(name));
        if let Some(ref parent) = style.parent {
            let _ = write_empty(w, "w:basedOn", &wval(parent.as_str()));
        }
        write_char_props_elem(w, &style.char_props);
        let _ = write_end(w, "w:style");
    }
}
