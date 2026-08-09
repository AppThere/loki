// SPDX-License-Identifier: Apache-2.0

//! The page form's **app-scoped** controls: "use this geometry for new
//! documents" (Spec 08 T6.3) and the measurement-unit picker (T6.4).
//!
//! # These two rows are not like the others
//!
//! Every other control in [`super::page_form`] edits *this document*. These
//! two write a settings file and change nothing on screen except the numbers'
//! units — so they are grouped and labelled apart, and the geometry one names
//! what it affects ("New docs") rather than saying "default", which reads as if
//! it were about the page in front of you.
//!
//! # Why a writer exists at all
//!
//! T6.3's store would otherwise have a reader and no writer for three of its
//! four fields: the seeding path consumes `page_size`, `margins` and
//! `measurement_unit`, and until these controls landed only `custom_sizes` was
//! ever written. A setting nothing can set is indistinguishable from one that
//! does not work.

use appthere_ui::tokens;
use dioxus::prelude::*;
use loki_doc_model::layout::page::PageLayout;
use loki_doc_model::loki_primitives::units::MeasurementUnit;
use loki_i18n::fl;

use super::super::editor_defaults::{self, PanelSettings};
use super::page_form::button_css;

/// A labelled row, matching the form's other rows.
fn row(label: String, controls: Element) -> Element {
    rsx! {
        div {
            style: "display: flex; flex-direction: row; align-items: center; gap: 6px; flex-wrap: wrap; margin-bottom: 4px;",
            span {
                style: format!(
                    "font-size: {fs}px; color: {fg}; min-width: 64px;",
                    fs = tokens::FONT_SIZE_LABEL,
                    fg = tokens::COLOR_TEXT_ON_CHROME_SECONDARY,
                ),
                { label }
            }
            {controls}
        }
    }
}

/// The measurement-unit picker: one button per unit, the active one marked.
///
/// Writing the unit re-renders the whole panel through `generation`, so the
/// inspector rows and the size field pick up the new unit together — they both
/// read it from `page_measurement_unit`, which is now backed by this.
///
/// # Touch target
///
/// Text buttons sharing [`button_css`]'s posture caveat.
pub(super) fn unit_row(active: MeasurementUnit, mut generation: Signal<u64>) -> Element {
    row(
        fl!("style-page-unit-label"),
        rsx! {
            for unit in MeasurementUnit::ALL.iter().copied() {
                button {
                    key: "{unit:?}",
                    style: button_css(unit == active),
                    onclick: move |_| {
                        editor_defaults::set_measurement_unit(unit);
                        generation += 1;
                    },
                    { unit.abbreviation() }
                }
            }
        },
    )
}

/// "Use this page's geometry for new documents", plus a reset.
///
/// The reset is only offered once something has been recorded — a control that
/// clears nothing is a control that looks like it did something.
pub(super) fn new_document_defaults_row(
    layout: &PageLayout,
    settings: &PanelSettings,
    mut generation: Signal<u64>,
) -> Element {
    let captured = layout.clone();
    let has_defaults = settings.has_page_geometry;
    row(
        fl!("style-page-defaults-label"),
        rsx! {
            button {
                style: button_css(false),
                onclick: move |_| {
                    editor_defaults::set_default_page_geometry(&captured);
                    generation += 1;
                },
                { fl!("style-page-set-default") }
            }
            if has_defaults {
                button {
                    style: button_css(false),
                    onclick: move |_| {
                        editor_defaults::clear_default_page_geometry();
                        generation += 1;
                    },
                    { fl!("style-page-clear-default") }
                }
            }
        },
    )
}
