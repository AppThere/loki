// SPDX-License-Identifier: Apache-2.0

//! The **Header** and **Footer** tabs.
//!
//! One module for both: they differ in three places — which band they size,
//! whether page numbering appears, and their labels — and two near-identical
//! files would drift on the first change made to only one of them.

use appthere_ui::{
    AtCheckRow, AtDialogNotice, AtField, AtNoticeTone, AtSegmented, DialogPosture,
    at_control_style, tokens,
};
use dioxus::prelude::*;
use loki_doc_model::layout::header_footer::{HeaderFooter, HeaderFooterKind};
use loki_doc_model::style::list_style::NumberingScheme;
use loki_i18n::fl;

use super::super::editor_defaults::PanelSettings;
use super::body::{PageDraft, grid_style, measure_field, section_heading, span_all};

/// The numbering schemes the footer offers, in display order.
const SCHEMES: [NumberingScheme; 5] = [
    NumberingScheme::Decimal,
    NumberingScheme::LowerRoman,
    NumberingScheme::UpperRoman,
    NumberingScheme::LowerAlpha,
    NumberingScheme::UpperAlpha,
];

/// The localized label for a numbering scheme.
fn scheme_label(scheme: NumberingScheme) -> String {
    match scheme {
        NumberingScheme::LowerRoman => fl!("page-dialog-number-lower-roman"),
        NumberingScheme::UpperRoman => fl!("page-dialog-number-upper-roman"),
        NumberingScheme::LowerAlpha => fl!("page-dialog-number-lower-alpha"),
        NumberingScheme::UpperAlpha => fl!("page-dialog-number-upper-alpha"),
        _ => fl!("page-dialog-number-decimal"),
    }
}

/// Renders the Header (`header == true`) or Footer tab body.
pub(super) fn body(
    draft: PageDraft,
    posture: DialogPosture,
    settings: &PanelSettings,
    header: bool,
) -> Element {
    let Some(current) = draft.read().clone() else {
        return rsx! {};
    };
    let l = &current.layout;
    let enabled = if header {
        l.header.is_some()
    } else {
        l.footer.is_some()
    };
    let differs_first = if header {
        l.header_first.is_some()
    } else {
        l.footer_first.is_some()
    };
    let differs_even = if header {
        l.header_even.is_some()
    } else {
        l.footer_even.is_some()
    };
    let scheme = l.page_number_format.unwrap_or(NumberingScheme::Decimal);
    let start = l.page_number_start.unwrap_or(1);

    rsx! {
        div {
            style: grid_style(posture),

            div {
                style: span_all(posture),
                AtCheckRow {
                    checked: enabled,
                    min_touch_px: posture.min_touch_px,
                    aria_label: if header { fl!("page-dialog-header-enable") } else { fl!("page-dialog-footer-enable") },
                    label: rsx! {
                        if header {
                            { fl!("page-dialog-header-enable") }
                        } else {
                            { fl!("page-dialog-footer-enable") }
                        }
                    },
                    on_toggle: move |v: bool| set_band(draft, header, v),
                }
            }

            // The geometry only means something once the band exists.
            { measure_field(
                if header { fl!("page-dialog-header-height") } else { fl!("page-dialog-footer-height") },
                draft, posture, settings,
                move |d| if header { d.buffers.header.clone() } else { d.buffers.footer.clone() },
                move |d, pt| {
                    if header {
                        d.layout.margins.header = pt;
                    } else {
                        d.layout.margins.footer = pt;
                    }
                },
                move |d, v| {
                    if header {
                        d.buffers.header = v;
                    } else {
                        d.buffers.footer = v;
                    }
                },
                if enabled { None } else { Some(fl!("page-dialog-band-disabled")) },
                !enabled,
                String::new(),
            ) }

            { section_heading(fl!("page-dialog-band-variants"), posture) }

            div {
                style: span_all(posture),
                AtCheckRow {
                    checked: !differs_first,
                    disabled: !enabled,
                    min_touch_px: posture.min_touch_px,
                    aria_label: fl!("page-dialog-same-first"),
                    label: rsx! { { fl!("page-dialog-same-first") } },
                    on_toggle: move |v: bool| set_variant(draft, header, Variant::First, !v),
                }
                AtCheckRow {
                    checked: !differs_even,
                    disabled: !enabled,
                    min_touch_px: posture.min_touch_px,
                    aria_label: fl!("page-dialog-same-even"),
                    label: rsx! { { fl!("page-dialog-same-even") } },
                    on_toggle: move |v: bool| set_variant(draft, header, Variant::Even, !v),
                }
            }

            // Page numbering lives on the footer: it is a per-page-style
            // property in the model, and offering it on both tabs would be one
            // value with two controls.
            if !header {
                { section_heading(fl!("page-dialog-numbering"), posture) }

                AtField {
                    label: fl!("page-dialog-number-format"),
                    control: rsx! {
                        AtSegmented {
                            options: SCHEMES.iter().map(|s| scheme_label(*s)).collect::<Vec<_>>(),
                            selected: SCHEMES.iter().position(|s| *s == scheme).unwrap_or(0),
                            min_touch_px: posture.min_touch_px,
                            on_select: move |idx: usize| {
                                let Some(s) = SCHEMES.get(idx).copied() else { return };
                                let mut draft = draft;
                                let mut next = draft.read().clone();
                                if let Some(d) = next.as_mut() {
                                    d.layout.page_number_format = Some(s);
                                }
                                draft.set(next);
                            },
                        }
                    },
                }

                AtField {
                    label: fl!("page-dialog-number-start"),
                    control: rsx! {
                        div {
                            style: at_control_style(posture.min_touch_px, "width: 100%;"),
                            input {
                                r#type: "text",
                                value: "{start}",
                                style: format!(
                                    "flex: 1; min-width: 0; background: transparent; border: none; \
                                     font-size: {fs}px; color: {fg};",
                                    fs = tokens::FONT_SIZE_MD,
                                    fg = tokens::COLOR_TEXT_ON_CHROME,
                                ),
                                oninput: move |evt| {
                                    let mut draft = draft;
                                    let mut next = draft.read().clone();
                                    if let Some(d) = next.as_mut() {
                                        let text = evt.value();
                                        // A blank box restores automatic
                                        // numbering rather than pinning page 0.
                                        d.layout.page_number_start = if text.trim().is_empty() {
                                            None
                                        } else {
                                            text.trim().parse::<u32>().ok().map(|n| n.max(1))
                                        };
                                    }
                                    draft.set(next);
                                },
                            }
                        }
                    },
                }
            }

            // Header and footer *content* is block content, edited on the page
            // rather than in a form: the model stores `Vec<Block>`, which a text
            // box cannot round-trip without discarding fields, styles and runs.
            AtDialogNotice {
                tone: AtNoticeTone::Info,
                extra_style: span_all(posture),
                message: rsx! { { fl!("page-dialog-band-content-note") } },
            }
        }
    }
}

/// Which alternate band a toggle addresses.
#[derive(Clone, Copy, PartialEq, Eq)]
enum Variant {
    /// The first page of the style.
    First,
    /// Even (verso) pages.
    Even,
}

/// Turns the header or footer band on or off.
fn set_band(mut draft: PageDraft, header: bool, on: bool) {
    let mut next = draft.read().clone();
    if let Some(d) = next.as_mut() {
        let band = on.then(|| HeaderFooter::new(HeaderFooterKind::Default));
        if header {
            d.layout.header = band;
        } else {
            d.layout.footer = band;
        }
    }
    draft.set(next);
}

/// Adds or clears an alternate band.
fn set_variant(mut draft: PageDraft, header: bool, variant: Variant, differs: bool) {
    let mut next = draft.read().clone();
    if let Some(d) = next.as_mut() {
        let band = differs.then(|| {
            HeaderFooter::new(match variant {
                Variant::First => HeaderFooterKind::First,
                Variant::Even => HeaderFooterKind::Even,
            })
        });
        match (header, variant) {
            (true, Variant::First) => d.layout.header_first = band,
            (true, Variant::Even) => d.layout.header_even = band,
            (false, Variant::First) => d.layout.footer_first = band,
            (false, Variant::Even) => d.layout.footer_even = band,
        }
    }
    draft.set(next);
}
