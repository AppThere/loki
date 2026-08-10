// SPDX-License-Identifier: Apache-2.0

//! The paper preview (design note 05).
//!
//! A real paper canvas — [`tokens::CANVAS_PAGE_BG`], serif, document ink — not a
//! dark-chrome swatch, because the question it answers is "what will this look
//! like on the page". It is side-docked at Expanded and a disclosure row below
//! 1024 px so the form never loses width.
//!
//! # It previews the draft, not the document
//!
//! The specimen is styled from the **staged** draft, so a change shows before
//! Apply. It is a specimen and not a live layout: it renders two paragraphs of
//! fixed text through the properties on this tab, rather than running
//! `loki-layout` over the real document. A true live render would need a layout
//! pass per keystroke on a document of arbitrary size, and it would answer a
//! different question — "what happened to page 34" — than the one a style
//! editor is asking.

use appthere_ui::tokens;
use dioxus::prelude::*;
use loki_doc_model::style::props::para_props::{LineHeight, ParagraphAlignment, Spacing};
use loki_doc_model::style::{ParagraphStyle, StyleCatalog, StyleId};
use loki_i18n::fl;

use super::fields::DraftSignal;
use super::rows::resolve_inherited;
use super::tabs::ParaTab;

/// The docked preview rail: heading plus the paper specimen.
pub(super) fn rail(
    tab: ParaTab,
    catalog: &StyleCatalog,
    id: &StyleId,
    draft: DraftSignal,
) -> Element {
    rsx! {
        div {
            style: format!(
                "display: flex; flex-direction: column; height: 100%; min-height: 0;",
            ),
            div {
                style: format!(
                    "flex-shrink: 0; padding: {py}px {px}px; \
                     border-bottom: 1px solid {border}; font-size: {fs}px; \
                     font-weight: {fw}; letter-spacing: 0.04em; \
                     text-transform: uppercase; color: {fg};",
                    py = tokens::SPACE_3,
                    px = tokens::SPACE_4,
                    border = tokens::COLOR_BORDER_CHROME,
                    fs = tokens::FONT_SIZE_LABEL,
                    fw = tokens::FONT_WEIGHT_SEMIBOLD,
                    fg = tokens::COLOR_TEXT_ON_CHROME_SECONDARY,
                ),
                { fl!("style-dialog-preview") }
            }
            div {
                style: format!("flex: 1; min-height: 0; padding: {}px;", tokens::SPACE_4),
                { specimen(tab, catalog, id, draft) }
            }
        }
    }
}

/// The paper specimen itself, styled from the staged draft.
pub(super) fn specimen(
    tab: ParaTab,
    catalog: &StyleCatalog,
    id: &StyleId,
    draft: DraftSignal,
) -> Element {
    let style = draft
        .read()
        .as_ref()
        .map(|d| d.style.clone())
        .or_else(|| catalog.paragraph_styles.get(id).cloned());
    let Some(style) = style else {
        return rsx! {};
    };
    let resolved = resolve_preview(catalog, &style);
    let heading = style
        .display_name
        .clone()
        .unwrap_or_else(|| id.as_str().to_string());
    // The Borders tab is the one place a paragraph rule is drawn inside the
    // specimen, so the preview shows the edge rather than describing it.
    let border_css = if tab == ParaTab::Borders {
        border_specimen_css(&style)
    } else {
        String::new()
    };

    rsx! {
        div {
            style: format!(
                "height: 100%; box-sizing: border-box; overflow: hidden; \
                 background: {bg}; color: {ink}; border-radius: {r}px; \
                 padding: {p}px; font-family: {serif}; font-size: {fs}px;",
                bg = tokens::CANVAS_PAGE_BG,
                ink = tokens::COLOR_TEXT_PRIMARY,
                r = tokens::RADIUS_SM,
                p = tokens::SPACE_4,
                serif = PREVIEW_SERIF,
                fs = PREVIEW_FONT_SIZE_PX,
            ),
            div {
                style: format!(
                    "margin-bottom: {mb}px; font-family: {ui}; font-size: {fs}px; \
                     letter-spacing: 0.06em; text-transform: uppercase; color: {fg};",
                    mb = tokens::SPACE_2,
                    ui = tokens::FONT_FAMILY_UI,
                    fs = tokens::FONT_SIZE_XS,
                    fg = tokens::COLOR_TEXT_SECONDARY,
                ),
                {heading}
            }
            p {
                style: format!("margin: 0; {border_css} {}", paragraph_css(&resolved)),
                { fl!("style-dialog-preview-body-1") }
            }
            p {
                style: format!(
                    "margin: 0; color: {fg}; {}",
                    paragraph_css(&resolved),
                    fg = tokens::COLOR_TEXT_SECONDARY,
                ),
                { fl!("style-dialog-preview-body-2") }
            }
        }
    }
}

/// The serif stack the specimen renders in — a bundled face, so the preview
/// shows a document face rather than the UI chrome typeface.
const PREVIEW_SERIF: &str = "Tinos, Gelasio, serif";

/// Specimen body size. Fixed rather than the style's own size: the rail is
/// 300 px wide, and a 24 pt heading style would render three words per line and
/// preview nothing useful. Relative sizes (indents, spacing) are honoured.
const PREVIEW_FONT_SIZE_PX: f32 = 13.0;

/// The paragraph properties the specimen honours, already resolved through the
/// chain so an inherited value previews the same as a local one.
struct PreviewProps {
    alignment: ParagraphAlignment,
    indent_start_pt: f64,
    indent_end_pt: f64,
    indent_first_pt: f64,
    space_before_pt: f64,
    space_after_pt: f64,
    line_height: f32,
}

/// Resolves the preview's properties, preferring the staged draft's local
/// values and falling back through the committed chain.
fn resolve_preview(catalog: &StyleCatalog, staged: &ParagraphStyle) -> PreviewProps {
    let pp = &staged.para_props;
    let points =
        |local: Option<loki_doc_model::loki_primitives::units::Points>,
         get: fn(&ParagraphStyle) -> Option<loki_doc_model::loki_primitives::units::Points>|
         -> f64 {
            local
                .or_else(|| resolve_inherited(catalog, staged, get))
                .map(|p| p.value())
                .unwrap_or(0.0)
        };
    let spacing = |local: Option<&Spacing>, get: fn(&ParagraphStyle) -> Option<Spacing>| -> f64 {
        let resolved = local
            .cloned()
            .or_else(|| resolve_inherited(catalog, staged, get));
        match resolved {
            Some(Spacing::Exact(pt)) => pt.value(),
            _ => 0.0,
        }
    };

    PreviewProps {
        alignment: pp
            .alignment
            .or_else(|| resolve_inherited(catalog, staged, |s| s.para_props.alignment))
            .unwrap_or(ParagraphAlignment::Left),
        indent_start_pt: points(pp.indent_start, |s| s.para_props.indent_start),
        indent_end_pt: points(pp.indent_end, |s| s.para_props.indent_end),
        indent_first_pt: points(pp.indent_first_line, |s| s.para_props.indent_first_line),
        space_before_pt: spacing(pp.space_before.as_ref(), |s| s.para_props.space_before),
        space_after_pt: spacing(pp.space_after.as_ref(), |s| s.para_props.space_after),
        line_height: pp
            .line_height
            .or_else(|| resolve_inherited(catalog, staged, |s| s.para_props.line_height))
            .and_then(|lh| match lh {
                LineHeight::Multiple(m) => Some(m),
                _ => None,
            })
            .unwrap_or(1.4),
    }
}

/// The specimen paragraph's CSS.
///
/// Points are scaled to the specimen's own size so an 18 pt indent reads as an
/// 18 pt indent *relative to the previewed text*, not as 18 CSS px beside 13 px
/// type — which would over-state every indent by roughly half.
fn paragraph_css(p: &PreviewProps) -> String {
    let scale = f64::from(PREVIEW_FONT_SIZE_PX) / 12.0;
    let px = |pt: f64| pt * scale;
    format!(
        "text-align: {align}; line-height: {lh}; \
         margin-left: {ml:.1}px; margin-right: {mr:.1}px; \
         margin-top: {mt:.1}px; margin-bottom: {mb:.1}px; \
         text-indent: {ti:.1}px;",
        align = css_align(p.alignment),
        lh = p.line_height,
        ml = px(p.indent_start_pt),
        mr = px(p.indent_end_pt),
        mt = px(p.space_before_pt),
        mb = px(p.space_after_pt),
        ti = px(p.indent_first_pt),
    )
}

/// Maps the model's alignment onto CSS.
///
/// `Distribute` renders as `justify`: CSS has no distributed alignment, and
/// showing it as `left` would preview a different paragraph from the one the
/// layout engine will produce.
fn css_align(a: ParagraphAlignment) -> &'static str {
    match a {
        ParagraphAlignment::Center => "center",
        ParagraphAlignment::Right => "right",
        ParagraphAlignment::Justify | ParagraphAlignment::Distribute => "justify",
        _ => "left",
    }
}

/// The border edges drawn on the Borders specimen, from the staged style.
fn border_specimen_css(style: &ParagraphStyle) -> String {
    let p = &style.para_props;
    let edge = |b: &Option<loki_doc_model::style::props::border::Border>, side: &str| -> String {
        match b {
            Some(border) => {
                let color = border
                    .color
                    .as_ref()
                    .and_then(loki_doc_model::loki_primitives::color::DocumentColor::to_hex)
                    .unwrap_or_else(|| tokens::COLOR_TEXT_PRIMARY.to_string());
                format!(
                    "border-{side}: {w:.1}px solid {color};",
                    w = border.width.value().max(0.5),
                )
            }
            None => String::new(),
        }
    };
    let pad = |pt: Option<loki_doc_model::loki_primitives::units::Points>, side: &str| -> String {
        pt.map(|v| format!("padding-{side}: {:.1}px;", v.value()))
            .unwrap_or_default()
    };
    format!(
        "{}{}{}{}{}{}{}{}",
        edge(&p.border_top, "top"),
        edge(&p.border_bottom, "bottom"),
        edge(&p.border_left, "left"),
        edge(&p.border_right, "right"),
        pad(p.padding_top, "top"),
        pad(p.padding_bottom, "bottom"),
        pad(p.padding_left, "left"),
        pad(p.padding_right, "right"),
    )
}
