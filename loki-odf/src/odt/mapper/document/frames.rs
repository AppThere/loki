// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 AppThere Loki contributors

//! ODF drawing frame → inline/block mapping.

use loki_doc_model::content::attr::NodeAttr;
use loki_doc_model::content::block::{Block, Caption};
use loki_doc_model::content::inline::{Inline, LinkTarget};

use crate::error::OdfWarning;
use crate::odt::model::frames::{OdfFrame, OdfFrameKind};

use super::context::OdfMappingContext;
use super::paragraphs::map_paragraph;

/// Map an ODF drawing frame to an inline element.
///
/// For `as-char`-anchored frames, the mapped element is returned directly.
/// For floating frames, a [`Block::Figure`] or [`Block::Div`] is pushed to
/// [`OdfMappingContext::pending_figures`] and `None` is returned.
pub(crate) fn map_frame(frame: &OdfFrame, ctx: &mut OdfMappingContext<'_>) -> Option<Inline> {
    use base64::Engine as _;
    let is_as_char = frame.anchor_type.as_deref() == Some("as-char");

    match &frame.kind {
        OdfFrameKind::Image {
            href,
            media_type,
            title,
            desc,
        } => {
            if !ctx.options.embed_images {
                return None;
            }
            let Some((stored_mt, bytes)) = ctx.images.get(href.as_str()) else {
                ctx.warnings
                    .push(OdfWarning::MissingImage { href: href.clone() });
                return None;
            };
            let b64 = base64::engine::general_purpose::STANDARD.encode(bytes);
            let mt = media_type.as_deref().unwrap_or(stored_mt.as_str());
            let data_uri = format!("data:{mt};base64,{b64}");
            let alt: Vec<Inline> = desc
                .as_deref()
                .or(title.as_deref())
                .map(|s| vec![Inline::Str(s.into())])
                .unwrap_or_default();
            let img = Inline::Image(NodeAttr::default(), alt, LinkTarget::new(data_uri));
            if is_as_char {
                Some(img)
            } else {
                ctx.pending_figures.push(Block::Figure(
                    NodeAttr::default(),
                    Caption::default(),
                    vec![Block::Para(vec![img])],
                ));
                None
            }
        }
        OdfFrameKind::TextBox { paragraphs } => {
            // Map the text box content as a Div pushed to pending_figures.
            let inner: Vec<Block> = paragraphs
                .iter()
                .flat_map(|p| {
                    let block = map_paragraph(p, ctx);
                    let figs = std::mem::take(&mut ctx.pending_figures);
                    std::iter::once(block).chain(figs)
                })
                .collect();
            ctx.pending_figures
                .push(Block::Div(NodeAttr::default(), inner));
            None
        }
        OdfFrameKind::Other => {
            ctx.warnings.push(OdfWarning::DroppedFrame {
                name: frame.name.clone(),
            });
            None
        }
    }
}
