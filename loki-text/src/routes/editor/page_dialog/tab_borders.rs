// SPDX-License-Identifier: Apache-2.0

//! The **Borders** tab: the page border and its background.

use appthere_ui::{AtDialogNotice, AtField, AtNoticeTone, AtSegmented, DialogPosture, tokens};
use dioxus::prelude::*;
use loki_doc_model::layout::page::PageBorders;
use loki_doc_model::loki_primitives::color::DocumentColor;
use loki_doc_model::loki_primitives::units::Points;
use loki_doc_model::style::props::border::{Border, BorderStyle};
use loki_i18n::fl;

use super::body::{PageDraft, grid_style, span_all};

/// The line styles offered, matching the paragraph Borders tab: the bevel
/// styles need two tones of a colour to read, which neither Blitz nor a 1-bit
/// E-Ink panel can paint.
const STYLES: [BorderStyle; 4] = [
    BorderStyle::Solid,
    BorderStyle::Dashed,
    BorderStyle::Dotted,
    BorderStyle::Double,
];

/// The default page-border width when one is first added.
const DEFAULT_WIDTH_PT: f64 = 0.5;

/// The localized label for a line style.
fn style_label(style: BorderStyle) -> String {
    match style {
        BorderStyle::Dashed => fl!("page-dialog-border-dashed"),
        BorderStyle::Dotted => fl!("page-dialog-border-dotted"),
        BorderStyle::Double => fl!("page-dialog-border-double"),
        _ => fl!("page-dialog-border-solid"),
    }
}

/// Renders the Borders tab body.
pub(super) fn body(draft: PageDraft, posture: DialogPosture) -> Element {
    let Some(current) = draft.read().clone() else {
        return rsx! {};
    };
    let borders = current.layout.page_border.clone();
    let has_border = borders.as_ref().is_some_and(|b| {
        b.top.is_some() || b.bottom.is_some() || b.left.is_some() || b.right.is_some()
    });
    let line_style = borders
        .as_ref()
        .and_then(|b| b.top.as_ref())
        .map(|b| b.style)
        .unwrap_or(BorderStyle::Solid);

    // Hoisted: a prop value is an expression position, and an `if` is not one.
    let style_note: Element = if has_border {
        rsx! {}
    } else {
        rsx! {
            div {
                style: format!(
                    "font-size: {fs}px; color: {fg};",
                    fs = tokens::FONT_SIZE_LABEL,
                    fg = tokens::COLOR_TEXT_ON_CHROME_SECONDARY,
                ),
                { fl!("page-dialog-border-none-note") }
            }
        }
    };

    rsx! {
        div {
            style: grid_style(posture),

            AtField {
                label: fl!("page-dialog-border-edges"),
                extra_style: span_all(posture),
                control: rsx! {
                    AtSegmented {
                        options: vec![
                            fl!("page-dialog-border-none"),
                            fl!("page-dialog-border-all"),
                        ],
                        selected: usize::from(has_border),
                        min_touch_px: posture.min_touch_px,
                        on_select: move |idx: usize| set_border(draft, idx == 1, line_style),
                    }
                },
            }

            AtField {
                label: fl!("page-dialog-border-style"),
                disabled: !has_border,
                control: rsx! {
                    AtSegmented {
                        options: STYLES.iter().map(|s| style_label(*s)).collect::<Vec<_>>(),
                        selected: STYLES.iter().position(|s| *s == line_style).unwrap_or(0),
                        disabled: !has_border,
                        min_touch_px: posture.min_touch_px,
                        on_select: move |idx: usize| {
                            let Some(s) = STYLES.get(idx).copied() else { return };
                            set_border(draft, true, s);
                        },
                    }
                },
                footnote: style_note,
            }

            // Page borders and backgrounds print but are not exported to EPUB —
            // stated here rather than discovered after a publish.
            AtDialogNotice {
                tone: AtNoticeTone::Info,
                extra_style: span_all(posture),
                message: rsx! { { fl!("page-dialog-border-export-note") } },
            }
        }
    }
}

/// Sets or clears the page border on all four edges.
fn set_border(mut draft: PageDraft, on: bool, style: BorderStyle) {
    let mut next = draft.read().clone();
    if let Some(d) = next.as_mut() {
        d.layout.page_border = on.then(|| {
            let edge = Border {
                style,
                width: Points::new(DEFAULT_WIDTH_PT),
                // `None` is automatic: the border takes the document's rule
                // colour, so it is not invisible in an inverted theme.
                color: None::<DocumentColor>,
                spacing: None,
            };
            PageBorders {
                top: Some(edge.clone()),
                left: Some(edge.clone()),
                bottom: Some(edge.clone()),
                right: Some(edge),
                offset_from_text: false,
            }
        });
    }
    draft.set(next);
}
