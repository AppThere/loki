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

//! Image/drawing mapper: `w:drawing` → [`Inline::Image`].

use loki_doc_model::content::attr::NodeAttr;
use loki_doc_model::content::inline::{Inline, LinkTarget};

use crate::docx::model::paragraph::DocxDrawing;
use crate::error::OoxmlWarning;

use super::document::MappingContext;

/// Maps a `w:drawing` element to an [`Inline::Image`].
///
/// Returns `None` and pushes a warning if the relationship id is missing
/// or cannot be resolved to an image part.
pub(crate) fn map_drawing(
    drawing: &DocxDrawing,
    ctx: &mut MappingContext<'_>,
) -> Option<Inline> {
    let rel_id = match &drawing.rel_id {
        Some(id) => id.clone(),
        None => {
            ctx.warnings.push(OoxmlWarning::UnresolvedImage { rel_id: String::new() });
            return None;
        }
    };

    let part = match ctx.images.get(&rel_id) {
        Some(p) => p,
        None => {
            ctx.warnings.push(OoxmlWarning::UnresolvedImage { rel_id });
            return None;
        }
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
    if let Some(cx) = drawing.cx {
        attr.kv.push(("cx_emu".to_string(), cx.to_string()));
    }
    if let Some(cy) = drawing.cy {
        attr.kv.push(("cy_emu".to_string(), cy.to_string()));
    }
    if drawing.is_anchor {
        attr.classes.push("floating".to_string());
    }

    let alt = match &drawing.descr {
        Some(d) if !d.is_empty() => vec![Inline::Str(d.clone())],
        _ => vec![],
    };

    let target = LinkTarget { url: uri, title: drawing.name.clone() };

    Some(Inline::Image(attr, alt, target))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashMap;
    use loki_doc_model::style::catalog::StyleCatalog;
    use loki_opc::PartData;
    use crate::docx::import::DocxImportOptions;

    fn make_ctx<'a>(
        images: &'a HashMap<String, PartData>,
        styles: &'a StyleCatalog,
        footnotes: &'a HashMap<i32, Vec<loki_doc_model::content::block::Block>>,
        endnotes: &'a HashMap<i32, Vec<loki_doc_model::content::block::Block>>,
        hyperlinks: &'a HashMap<String, String>,
        options: &'a DocxImportOptions,
    ) -> MappingContext<'a> {
        MappingContext {
            styles,
            footnotes,
            endnotes,
            hyperlinks,
            images,
            options,
            warnings: Vec::new(),
        }
    }

    #[test]
    fn missing_rel_id_returns_none_with_warning() {
        let images = HashMap::new();
        let catalog = StyleCatalog::default();
        let fn_map = HashMap::new();
        let en_map = HashMap::new();
        let hl_map = HashMap::new();
        let opts = DocxImportOptions::default();
        let mut ctx = make_ctx(&images, &catalog, &fn_map, &en_map, &hl_map, &opts);

        let drawing = DocxDrawing {
            rel_id: None,
            cx: None,
            cy: None,
            descr: None,
            name: None,
            is_anchor: false,
        };
        let result = map_drawing(&drawing, &mut ctx);
        assert!(result.is_none());
        assert_eq!(ctx.warnings.len(), 1);
        assert!(matches!(
            &ctx.warnings[0],
            OoxmlWarning::UnresolvedImage { rel_id } if rel_id.is_empty()
        ));
    }

    #[test]
    fn unresolved_rel_id_returns_none_with_warning() {
        let images = HashMap::new();
        let catalog = StyleCatalog::default();
        let fn_map = HashMap::new();
        let en_map = HashMap::new();
        let hl_map = HashMap::new();
        let opts = DocxImportOptions::default();
        let mut ctx = make_ctx(&images, &catalog, &fn_map, &en_map, &hl_map, &opts);

        let drawing = DocxDrawing {
            rel_id: Some("rId99".into()),
            cx: None,
            cy: None,
            descr: None,
            name: None,
            is_anchor: false,
        };
        let result = map_drawing(&drawing, &mut ctx);
        assert!(result.is_none());
        assert!(matches!(
            &ctx.warnings[0],
            OoxmlWarning::UnresolvedImage { rel_id } if rel_id == "rId99"
        ));
    }

    #[test]
    fn resolved_image_no_embed_returns_rel_id_as_url() {
        let mut images = HashMap::new();
        images.insert("rId1".into(), PartData::new(vec![0u8, 1, 2], "image/png"));

        let catalog = StyleCatalog::default();
        let fn_map = HashMap::new();
        let en_map = HashMap::new();
        let hl_map = HashMap::new();
        let opts = DocxImportOptions { embed_images: false, ..Default::default() };
        let mut ctx = make_ctx(&images, &catalog, &fn_map, &en_map, &hl_map, &opts);

        let drawing = DocxDrawing {
            rel_id: Some("rId1".into()),
            cx: Some(914400),
            cy: Some(685800),
            descr: Some("A test image".into()),
            name: Some("img1".into()),
            is_anchor: false,
        };
        let result = map_drawing(&drawing, &mut ctx).unwrap();
        if let Inline::Image(attr, alt, target) = result {
            assert_eq!(target.url, "rId1");
            assert_eq!(target.title.as_deref(), Some("img1"));
            assert!(matches!(&alt[..], [Inline::Str(s)] if s == "A test image"));
            assert!(attr.kv.iter().any(|(k, v)| k == "cx_emu" && v == "914400"));
            assert!(attr.kv.iter().any(|(k, v)| k == "cy_emu" && v == "685800"));
            assert!(!attr.classes.contains(&"floating".to_string()));
        } else {
            panic!("expected Image inline");
        }
    }

    #[test]
    fn anchor_drawing_gets_floating_class() {
        let mut images = HashMap::new();
        images.insert("rId2".into(), PartData::new(vec![], "image/jpeg"));

        let catalog = StyleCatalog::default();
        let fn_map = HashMap::new();
        let en_map = HashMap::new();
        let hl_map = HashMap::new();
        let opts = DocxImportOptions { embed_images: false, ..Default::default() };
        let mut ctx = make_ctx(&images, &catalog, &fn_map, &en_map, &hl_map, &opts);

        let drawing = DocxDrawing {
            rel_id: Some("rId2".into()),
            cx: None, cy: None, descr: None, name: None,
            is_anchor: true,
        };
        let result = map_drawing(&drawing, &mut ctx).unwrap();
        if let Inline::Image(attr, _, _) = result {
            assert!(attr.classes.contains(&"floating".to_string()));
        } else {
            panic!("expected Image");
        }
    }
}
