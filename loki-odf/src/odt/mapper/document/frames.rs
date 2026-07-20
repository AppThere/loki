// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 AppThere Loki contributors

//! Drawing-frame mapping: images (inline / floating), text boxes, and embedded
//! objects (formulas).

use loki_doc_model::content::attr::NodeAttr;
use loki_doc_model::content::block::{Block, Caption};
use loki_doc_model::content::float::{FloatWrap, TextWrap, WrapSide};
use loki_doc_model::content::inline::{Inline, LinkTarget, MathType};

use crate::error::OdfWarning;
use crate::odt::model::frames::{OdfFrame, OdfFrameKind};
use crate::odt::model::styles::OdfGraphicWrap;
use crate::xml_util::parse_length;

use super::OdfMappingContext;
use super::inlines::map_paragraph;

/// Convert an [`OdfGraphicWrap`] (raw `style:wrap`/`style:run-through`) to the
/// neutral [`FloatWrap`]. Returns `None` when no `style:wrap` is specified.
pub(super) fn map_graphic_wrap(gw: &OdfGraphicWrap) -> Option<FloatWrap> {
    let wrap_str = gw.wrap.as_deref()?;
    let (wrap, side) = match wrap_str {
        "none" => (TextWrap::TopAndBottom, WrapSide::Both),
        "parallel" => (TextWrap::Square, WrapSide::Both),
        "left" => (TextWrap::Square, WrapSide::Left),
        "right" => (TextWrap::Square, WrapSide::Right),
        "dynamic" | "biggest" => (TextWrap::Square, WrapSide::Largest),
        "run-through" => (TextWrap::None, WrapSide::Both),
        _ => return None,
    };
    let behind_text = gw.run_through.as_deref() == Some("background");
    Some(FloatWrap {
        wrap,
        side,
        behind_text,
    })
}

/// Resolve the text-wrap for a frame from its graphic style (`draw:style-name`).
fn frame_wrap(frame: &OdfFrame, ctx: &OdfMappingContext<'_>) -> Option<FloatWrap> {
    frame
        .style_name
        .as_deref()
        .and_then(|name| ctx.frame_wraps.get(name))
        .copied()
}

/// Builds the image's [`NodeAttr`], carrying the frame's pixel size as
/// `cx_emu`/`cy_emu` (parsed from `svg:width`/`svg:height`).
///
/// Without these, the layout engine treats the image as zero-sized and skips
/// it (and `plan_float` cannot reserve a wrap band). 1 pt = 12700 EMU.
fn image_attr_with_size(frame: &OdfFrame) -> NodeAttr {
    let mut attr = NodeAttr::default();
    let to_emu = |s: &Option<String>| -> Option<u64> {
        let pt = parse_length(s.as_deref()?)?.value();
        // Guarded positive and finite; image dimensions are far within u64 range,
        // so the round-and-cast cannot truncate or lose sign in practice.
        #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
        (pt > 0.0 && pt.is_finite()).then(|| (pt * 12700.0).round() as u64)
    };
    if let Some(cx) = to_emu(&frame.width) {
        attr.kv.push(("cx_emu".to_string(), cx.to_string()));
    }
    if let Some(cy) = to_emu(&frame.height) {
        attr.kv.push(("cy_emu".to_string(), cy.to_string()));
    }
    attr
}

/// Map an ODF drawing frame to an inline element.
///
/// `as-char` frames map to an inline image directly. A non-`as-char` (floating)
/// image frame that carries a text-wrap style is emitted as a **floating inline
/// image** (the wrap mode + [`FLOATING_CLASS`] stored on its attr) so the layout
/// engine flows text around it, exactly like the DOCX path. Floating frames
/// without a wrap (and text boxes / objects) are pushed to
/// [`OdfMappingContext::pending_figures`] as a block and `None` is returned.
///
/// [`FLOATING_CLASS`]: loki_doc_model::content::float::FLOATING_CLASS
pub(super) fn map_frame(frame: &OdfFrame, ctx: &mut OdfMappingContext<'_>) -> Option<Inline> {
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
            let mut attr = image_attr_with_size(frame);

            let wrap = (!is_as_char).then(|| frame_wrap(frame, ctx)).flatten();
            match wrap {
                // Floating image with a wrap: keep it inline (carrying the wrap)
                // so the flow engine reserves a band and wraps text beside it.
                Some(wrap) => {
                    wrap.store(&mut attr);
                    Some(Inline::Image(attr, alt, LinkTarget::new(data_uri)))
                }
                // as-char image: a plain inline image.
                None if is_as_char => Some(Inline::Image(attr, alt, LinkTarget::new(data_uri))),
                // Floating frame without a wrap style: block-stacked figure.
                None => {
                    let img = Inline::Image(attr, alt, LinkTarget::new(data_uri));
                    ctx.pending_figures.push(Block::Figure(
                        NodeAttr::default(),
                        Caption::default(),
                        vec![Block::Para(vec![img])],
                    ));
                    None
                }
            }
        }
        OdfFrameKind::TextBox { paragraphs } => {
            let inner: Vec<Block> = paragraphs
                .iter()
                .flat_map(|p| {
                    let block = map_paragraph(p, ctx);
                    let figs = std::mem::take(&mut ctx.pending_figures);
                    std::iter::once(block).chain(figs)
                })
                .collect();
            // A floating text box (one carrying a wrap style) maps to
            // `Inline::TextBox` so the flow engine renders it as a bordered box
            // with side-wrap — the same shape as the DOCX `wps` path, so a text
            // box round-trips DOCX ↔ ODT. Fill/border come from the frame's
            // graphic style; geometry from `svg:width`/`svg:height`. A text box
            // with no wrap stays a block-stacked `Div` (unchanged).
            let wrap = (!is_as_char).then(|| frame_wrap(frame, ctx)).flatten();
            if let Some(wrap) = wrap {
                let mut attr = image_attr_with_size(frame);
                wrap.store(&mut attr);
                let by_style = |m: &std::collections::HashMap<String, String>| {
                    frame.style_name.as_deref().and_then(|n| m.get(n)).cloned()
                };
                if let Some(fill) = by_style(ctx.frame_fills) {
                    attr.kv.push(("textbox-fill".to_string(), fill));
                }
                if let Some(line) = by_style(ctx.frame_strokes) {
                    attr.kv.push(("textbox-line".to_string(), line));
                }
                return Some(Inline::TextBox(attr, inner));
            }
            ctx.pending_figures
                .push(Block::Div(NodeAttr::default(), inner));
            None
        }
        OdfFrameKind::Object { href } => {
            // Resolve the embedded sub-document; if it is a MathML formula, map
            // it to inline math. ODF does not distinguish display from inline
            // math, so an embedded formula always maps to `MathType::InlineMath`.
            // Any other (or unresolvable) object — OLE, chart, … — is dropped
            // with a warning rather than silently lost.
            let key = href.trim_start_matches("./").trim_end_matches('/');
            let mathml = ctx
                .objects
                .get(key)
                .and_then(|bytes| crate::odt::math::extract_mathml(bytes));
            if let Some(mathml) = mathml {
                Some(Inline::Math(MathType::InlineMath, mathml))
            } else {
                ctx.warnings.push(OdfWarning::DroppedFrame {
                    name: frame.name.clone(),
                });
                None
            }
        }
        OdfFrameKind::Other => {
            ctx.warnings.push(OdfWarning::DroppedFrame {
                name: frame.name.clone(),
            });
            None
        }
    }
}
