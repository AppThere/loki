// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 AppThere Loki contributors

//! Image/drawing mapper: `w:drawing` → [`Inline::Image`].

use loki_doc_model::content::attr::NodeAttr;
use loki_doc_model::content::float::FLOATING_CLASS;
use loki_doc_model::content::inline::{Inline, LinkTarget};

use crate::docx::model::paragraph::DocxDrawing;
use crate::error::OoxmlWarning;

use super::document::MappingContext;

/// Store an anchored drawing's geometry, float, and wrap metadata onto `attr`
/// (shared by the image and text-box paths).
fn store_anchor_geometry(drawing: &DocxDrawing, attr: &mut NodeAttr) {
    if let Some(cx) = drawing.cx {
        attr.kv.push(("cx_emu".to_string(), cx.to_string()));
    }
    if let Some(cy) = drawing.cy {
        attr.kv.push(("cy_emu".to_string(), cy.to_string()));
    }
    if let Some(wrap) = drawing.wrap {
        wrap.store(attr);
    } else if drawing.is_anchor {
        attr.classes.push(FLOATING_CLASS.to_string());
    }
}

/// Maps a `wps` text-box drawing (one carrying `w:txbxContent`) to an
/// [`Inline::TextBox`]: its inner paragraphs become the box's block content, and
/// its geometry / wrap / fill / border ride on the [`NodeAttr`].
fn map_textbox(drawing: &DocxDrawing, ctx: &mut MappingContext<'_>) -> Inline {
    let blocks = drawing
        .txbx
        .iter()
        .flat_map(|p| crate::docx::mapper::paragraph::map_paragraph(p, ctx))
        .collect();
    let mut attr = NodeAttr::default();
    store_anchor_geometry(drawing, &mut attr);
    if let Some(fill) = &drawing.fill_color {
        attr.kv.push(("textbox-fill".to_string(), fill.clone()));
    }
    if let Some(line) = &drawing.line_color {
        attr.kv.push(("textbox-line".to_string(), line.clone()));
    }
    Inline::TextBox(attr, blocks)
}

/// Maps a `w:drawing` element to an [`Inline::Image`] or, when it carries
/// `w:txbxContent`, an [`Inline::TextBox`].
///
/// Returns `None` and pushes a warning if the relationship id is missing
/// or cannot be resolved to an image part.
pub(crate) fn map_drawing(drawing: &DocxDrawing, ctx: &mut MappingContext<'_>) -> Option<Inline> {
    if !drawing.txbx.is_empty() {
        return Some(map_textbox(drawing, ctx));
    }
    let Some(rel_id) = drawing.rel_id.clone() else {
        ctx.warnings.push(OoxmlWarning::UnresolvedImage {
            rel_id: String::new(),
        });
        return None;
    };

    let Some(part) = ctx.images.get(&rel_id) else {
        ctx.warnings.push(OoxmlWarning::UnresolvedImage { rel_id });
        return None;
    };

    let uri = if ctx.options.embed_images {
        use base64::Engine;
        format!(
            "data:{};base64,{}",
            part.media_type,
            base64::engine::general_purpose::STANDARD.encode(&part.bytes),
        )
    } else {
        rel_id.clone()
    };

    let mut attr = NodeAttr::default();
    store_anchor_geometry(drawing, &mut attr);

    let alt = match &drawing.descr {
        Some(d) if !d.is_empty() => vec![Inline::Str(d.clone())],
        _ => vec![],
    };

    let target = LinkTarget {
        url: uri,
        title: drawing.name.clone(),
    };

    Some(Inline::Image(attr, alt, target))
}

#[cfg(test)]
#[path = "images_tests.rs"]
mod tests;
