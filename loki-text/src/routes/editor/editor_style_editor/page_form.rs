// SPDX-License-Identifier: Apache-2.0

//! Editable **page-style** form (Spec 05 M6 page family, ADR-0012 Decision 2).
//!
//! LibreOffice-style per-page-style editing: preset buttons (orientation / size /
//! margins / columns, matching the Layout ribbon) that apply to **only the
//! selected page style's sections**. Each click computes the target section
//! indices + the edited [`PageLayout`] and writes them through
//! `set_page_style_geometry`, so the other page styles are untouched.
//!
//! [`apply_preset`] — the pure layout transform — is unit-tested; the component
//! is a thin applier.

use std::sync::{Arc, Mutex};

use appthere_ui::tokens;
use dioxus::prelude::*;
use loki_doc_model::layout::page::{PageLayout, PageOrientation, PageSize, SectionColumns};
use loki_doc_model::loki_primitives::units::Points;
use loki_doc_model::{rename_page_style, set_page_style_geometry};
use loki_i18n::fl;

use super::super::editor_keydown_ctrl::post_mutation_sync;
use super::StyleEditorSync;
use super::page_rename::PageRenameField;
use super::panel_data::page_edit_target;
use crate::editing::state::{DocumentState, apply_mutation_and_relayout};

/// A page-geometry preset the form can apply to a page style.
#[derive(Clone, Copy, PartialEq)]
pub(super) enum PagePreset {
    Portrait,
    Landscape,
    SizeA4,
    SizeLetter,
    MarginsNormal,
    MarginsNarrow,
    MarginsWide,
    Columns(u8),
}

/// The default inter-column gap when the form first adds columns (0.5 in),
/// matching the Layout ribbon.
const DEFAULT_COL_GAP_PT: f64 = 36.0;

/// Returns `current` with `preset` applied — the pure page-geometry transform.
/// Orientation and size preserve the other axis (choosing A4 while landscape
/// stays landscape); margins keep header/footer/gutter; columns keep the gap.
#[must_use]
pub(super) fn apply_preset(current: &PageLayout, preset: PagePreset) -> PageLayout {
    let mut l = current.clone();
    let is_landscape = l.page_size.width.value() > l.page_size.height.value();
    match preset {
        PagePreset::Portrait | PagePreset::Landscape => {
            let want = preset == PagePreset::Landscape;
            l.orientation = if want {
                PageOrientation::Landscape
            } else {
                PageOrientation::Portrait
            };
            if is_landscape != want {
                let (w, h) = (l.page_size.width, l.page_size.height);
                l.page_size = PageSize {
                    width: h,
                    height: w,
                };
            }
        }
        PagePreset::SizeA4 | PagePreset::SizeLetter => {
            let base = if preset == PagePreset::SizeA4 {
                PageSize::a4()
            } else {
                PageSize::letter()
            };
            l.page_size = if is_landscape {
                PageSize {
                    width: base.height,
                    height: base.width,
                }
            } else {
                base
            };
        }
        PagePreset::MarginsNormal | PagePreset::MarginsNarrow | PagePreset::MarginsWide => {
            let (tb, lr) = match preset {
                PagePreset::MarginsNarrow => (36.0, 36.0),
                PagePreset::MarginsWide => (72.0, 144.0),
                _ => (72.0, 72.0),
            };
            l.margins.top = Points::new(tb);
            l.margins.bottom = Points::new(tb);
            l.margins.left = Points::new(lr);
            l.margins.right = Points::new(lr);
        }
        PagePreset::Columns(n) => {
            l.columns = if n <= 1 {
                None
            } else {
                let gap = l
                    .columns
                    .as_ref()
                    .map_or(Points::new(DEFAULT_COL_GAP_PT), |c| c.gap);
                Some(SectionColumns {
                    count: n,
                    gap,
                    separator: l.columns.as_ref().is_some_and(|c| c.separator),
                })
            };
        }
    }
    l
}

/// Whether `layout` already matches `preset` (drives the active-button styling).
fn is_active(layout: &PageLayout, preset: PagePreset) -> bool {
    let landscape = layout.page_size.width.value() > layout.page_size.height.value();
    let (w, h) = (
        layout.page_size.width.value(),
        layout.page_size.height.value(),
    );
    let (short, long) = (w.min(h), w.max(h));
    let size_is = |p: &PageSize| {
        let (pw, ph) = (p.width.value(), p.height.value());
        (short - pw.min(ph)).abs() < 1.0 && (long - pw.max(ph)).abs() < 1.0
    };
    let m = &layout.margins;
    let all = |v: f64| (m.top.value() - v).abs() < 0.5 && (m.bottom.value() - v).abs() < 0.5;
    let lr = |v: f64| (m.left.value() - v).abs() < 0.5 && (m.right.value() - v).abs() < 0.5;
    let count = layout.columns.as_ref().map_or(1, |c| c.count);
    match preset {
        PagePreset::Portrait => !landscape,
        PagePreset::Landscape => landscape,
        PagePreset::SizeA4 => size_is(&PageSize::a4()),
        PagePreset::SizeLetter => size_is(&PageSize::letter()),
        PagePreset::MarginsNormal => all(72.0),
        PagePreset::MarginsNarrow => all(36.0),
        PagePreset::MarginsWide => all(72.0) && lr(144.0),
        PagePreset::Columns(n) => count == n,
    }
}

/// One preset button. Applies `preset` to the selected page style on click.
fn preset_button(
    doc_state: &Arc<Mutex<DocumentState>>,
    name: String,
    sync: StyleEditorSync,
    label: String,
    preset: PagePreset,
    active: bool,
) -> Element {
    let ds = Arc::clone(doc_state);
    rsx! {
        button {
            style: format!(
                "padding: 2px 6px; border-radius: 3px; border: 1px solid {border}; \
                 cursor: pointer; font-family: {ff}; font-size: {fs}px; \
                 background: {bg}; color: {fg};",
                border = if active { tokens::COLOR_TAB_ACTIVE_INDICATOR } else { tokens::COLOR_BORDER_CHROME },
                ff = tokens::FONT_FAMILY_UI,
                fs = tokens::FONT_SIZE_LABEL,
                bg = if active { tokens::COLOR_SURFACE_3 } else { tokens::COLOR_SURFACE_2 },
                fg = tokens::COLOR_TEXT_ON_CHROME,
            ),
            onclick: move |_| {
                let Some((current, indices)) = page_edit_target(&ds, &name) else {
                    return;
                };
                if indices.is_empty() {
                    return;
                }
                let next = apply_preset(&current, preset);
                let guard = sync.loro_doc.read();
                let Some(ldoc) = guard.as_ref() else { return };
                if set_page_style_geometry(ldoc, &indices, &next).is_ok() {
                    apply_mutation_and_relayout(&ds, ldoc);
                    post_mutation_sync(
                        &ds,
                        sync.loro_doc,
                        sync.cursor_state,
                        sync.undo_manager,
                        sync.can_undo,
                        sync.can_redo,
                    );
                }
            },
            "{label}"
        }
    }
}

/// A labelled row of preset buttons.
fn preset_row(label: String, buttons: Element) -> Element {
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
            {buttons}
        }
    }
}

/// The editable page-style form for the page style `name` (with its current
/// `layout` for active-state styling).
pub(super) fn page_style_form(
    doc_state: &Arc<Mutex<DocumentState>>,
    name: String,
    layout: PageLayout,
    mut editing_page_style: Signal<Option<String>>,
    sync: StyleEditorSync,
) -> Element {
    let btn = |label: String, preset: PagePreset| {
        preset_button(
            doc_state,
            name.clone(),
            sync,
            label,
            preset,
            is_active(&layout, preset),
        )
    };
    // The rename callback: commit the new name through `rename_page_style`, sync
    // undo/redo, and re-select the style under its new name so the panel keeps it
    // open. Captured by value so the returned rsx owns everything it needs.
    let ds_rename = Arc::clone(doc_state);
    let old_name = name.clone();
    let on_rename = move |new: String| {
        let guard = sync.loro_doc.read();
        let Some(ldoc) = guard.as_ref() else { return };
        if rename_page_style(ldoc, &old_name, &new).is_ok() {
            apply_mutation_and_relayout(&ds_rename, ldoc);
            drop(guard);
            post_mutation_sync(
                &ds_rename,
                sync.loro_doc,
                sync.cursor_state,
                sync.undo_manager,
                sync.can_undo,
                sync.can_redo,
            );
            editing_page_style.set(Some(new));
        }
    };
    rsx! {
        div {
            style: format!("display: flex; flex-direction: column; padding: {}px;", tokens::SPACE_2),

            PageRenameField { key: "{name}", name: name.clone(), on_rename }

            { preset_row(fl!("style-page-orientation"), rsx! {
                { btn(fl!("ribbon-orientation-portrait-aria"), PagePreset::Portrait) }
                { btn(fl!("ribbon-orientation-landscape-aria"), PagePreset::Landscape) }
            }) }
            { preset_row(fl!("style-page-size"), rsx! {
                { btn(fl!("ribbon-page-a4-aria"), PagePreset::SizeA4) }
                { btn(fl!("ribbon-page-letter-aria"), PagePreset::SizeLetter) }
            }) }
            { preset_row(fl!("style-page-margins"), rsx! {
                { btn(fl!("ribbon-margin-normal-aria"), PagePreset::MarginsNormal) }
                { btn(fl!("ribbon-margin-narrow-aria"), PagePreset::MarginsNarrow) }
                { btn(fl!("ribbon-margin-wide-aria"), PagePreset::MarginsWide) }
            }) }
            { preset_row(fl!("style-page-columns"), rsx! {
                { btn(fl!("ribbon-columns-one-aria"), PagePreset::Columns(1)) }
                { btn(fl!("ribbon-columns-two-aria"), PagePreset::Columns(2)) }
                { btn(fl!("ribbon-columns-three-aria"), PagePreset::Columns(3)) }
            }) }
        }
    }
}

#[cfg(test)]
#[path = "page_form_tests.rs"]
mod tests;
