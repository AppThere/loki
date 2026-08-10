// SPDX-License-Identifier: Apache-2.0

//! The paper thumbnail (design section 3's preview rail).
//!
//! A **scaled outline of the real page**, not a stock illustration: the sheet's
//! aspect ratio is the page's, the dashed frame sits at the real margins, and
//! the caption states the resulting text area. A page whose margins do not fit
//! its paper therefore looks wrong here, which is the point.

use appthere_ui::tokens;
use dioxus::prelude::*;
use loki_doc_model::layout::page::PageLayout;
use loki_doc_model::loki_primitives::units::Points;
use loki_i18n::fl;

use super::super::editor_defaults::PanelSettings;
use super::body::PageDraft;
use super::tabs::{is_mirrored, unit_label};

/// The thumbnail's longest edge, in CSS px.
const SHEET_MAX_PX: f32 = 230.0;

/// The rail: heading, sheet, caption.
pub(super) fn rail(draft: PageDraft, settings: &PanelSettings) -> Element {
    let Some(current) = draft.read().clone() else {
        return rsx! {};
    };
    let layout = current.layout.clone();

    rsx! {
        div {
            style: "display: flex; flex-direction: column; height: 100%; min-height: 0;",
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
                { fl!("page-dialog-preview") }
            }
            div {
                style: format!(
                    "flex: 1; min-height: 0; padding: {p}px; display: flex; \
                     flex-direction: column; align-items: center; gap: {gap}px;",
                    p = tokens::SPACE_5,
                    gap = tokens::SPACE_4,
                ),
                { sheet(&layout) }
                div {
                    style: format!(
                        "text-align: center; font-size: {fs}px; line-height: 1.5; color: {fg};",
                        fs = tokens::FONT_SIZE_LABEL,
                        fg = tokens::COLOR_TEXT_ON_CHROME_SECONDARY,
                    ),
                    { caption(&layout, settings) }
                }
            }
        }
    }
}

/// The sheet itself, with the text area drawn at the real margins.
fn sheet(layout: &PageLayout) -> Element {
    let w = layout.page_size.width.value().max(1.0);
    let h = layout.page_size.height.value().max(1.0);
    // Scale by the longer edge so landscape and portrait both fit the rail.
    let scale = f64::from(SHEET_MAX_PX) / w.max(h);
    let (sheet_w, sheet_h) = (w * scale, h * scale);
    let m = &layout.margins;
    // The gutter widens the binding edge, so it belongs to the start margin —
    // drawing it at zero would show a text area the paginator will not produce.
    let pad_start = (m.left.value() + m.gutter.value()) * scale;

    rsx! {
        div {
            style: format!(
                "width: {sheet_w:.1}px; height: {sheet_h:.1}px; box-sizing: border-box; \
                 background: {bg}; border-radius: {r}px; \
                 padding: {top:.1}px {right:.1}px {bottom:.1}px {left:.1}px;",
                bg = tokens::CANVAS_PAGE_BG,
                r = tokens::RADIUS_SM,
                top = m.top.value() * scale,
                right = m.right.value() * scale,
                bottom = m.bottom.value() * scale,
                left = pad_start,
            ),
            div {
                style: format!(
                    "width: 100%; height: 100%; box-sizing: border-box; \
                     border: 1px dashed {rule}; display: flex; flex-direction: row; gap: {gap:.1}px;",
                    rule = tokens::COLOR_BORDER_DEFAULT,
                    gap = layout.columns.as_ref().map_or(0.0, |c| c.gap.value() * scale),
                ),
                // One block per column, so a two-column page previews as two.
                for i in 0..layout.columns.as_ref().map_or(1, |c| usize::from(c.count.max(1))) {
                    div {
                        key: "col-{i}",
                        style: format!(
                            "flex: 1; min-width: 0; background: {fill};",
                            fill = tokens::COLOR_BORDER_DEFAULT,
                        ),
                    }
                }
            }
        }
    }
}

/// The line under the sheet: paper, orientation, mirroring, and the text area.
fn caption(layout: &PageLayout, settings: &PanelSettings) -> String {
    let u = settings.unit;
    let m = &layout.margins;
    let text_w = Points::new(
        (layout.page_size.width.value() - m.left.value() - m.right.value() - m.gutter.value())
            .max(0.0),
    );
    let text_h =
        Points::new((layout.page_size.height.value() - m.top.value() - m.bottom.value()).max(0.0));
    let landscape = layout.page_size.width.value() > layout.page_size.height.value();

    fl!(
        "page-dialog-preview-caption",
        orientation = if landscape {
            fl!("page-dialog-landscape")
        } else {
            fl!("page-dialog-portrait")
        },
        mirrored = if is_mirrored(layout) {
            fl!("page-dialog-preview-mirrored")
        } else {
            fl!("page-dialog-preview-not-mirrored")
        },
        width = u.format_bare(text_w),
        height = u.format_bare(text_h),
        unit = unit_label(u)
    )
}
