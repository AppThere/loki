// SPDX-License-Identifier: Apache-2.0

//! Editable **page-style** form (Spec 05 M6 page family, ADR-0012 Decision 2).
//!
//! LibreOffice-style per-page-style editing: preset buttons (orientation / size /
//! margins / columns, matching the Layout ribbon) plus a column-count stepper and
//! separator toggle, all applying to **only the selected page style** — the
//! mutation resolves the sections that reference it, so the other page styles are
//! untouched.
//!
//! The form also carries the family's two manager verbs: renaming the selected
//! style ([`super::page_rename`]) and **applying** it to the section the caret is
//! in ([`set_section_page_style`]). Creating one lives in
//! [`super::page_browser`], next to the list it appears in.
//!
//! [`apply_preset`] — the pure layout transform — is unit-tested in
//! [`super::page_presets`]; this module is a thin applier.

use std::sync::{Arc, Mutex};

use appthere_ui::tokens;
use dioxus::prelude::*;
use loki_doc_model::layout::page::{PageLayout, PageSize};
use loki_doc_model::{rename_page_style, set_page_style_geometry, set_section_page_style};
use loki_i18n::fl;

use super::super::editor_keydown_ctrl::post_mutation_sync;
use super::StyleEditorSync;
use super::page_defaults_row::{new_document_defaults_row, unit_row};
use super::page_presets::{PagePreset, apply_preset, column_count, is_active};
use super::page_rename::PageRenameField;
use super::page_size_picker::size_section;
use super::panel_data_page::{caret_section_index, page_edit_target, page_measurement_unit};
use crate::editing::state::{DocumentState, apply_mutation_and_relayout};

/// Shared button chrome; `active` gives the pressed/selected look.
///
/// # Touch target
///
/// These are text buttons inside the style panel: 2×6 px padding around a
/// `FONT_SIZE_LABEL` glyph run, so the intrinsic box is under the WCAG 2.5.8
/// 44×44 logical-pixel minimum at the desktop font scale. They meet it the same
/// way the panel's other controls do — on touch builds the base font scale
/// lifts the whole panel — and the Compact posture's `touch_min_css()` is what
/// enforces it for the list rows. **The stepper's `−`/`+` are the smallest
/// targets in the panel** and are the ones to re-measure when the panel next
/// gets a screen-sitting scenario; they are not covered by `touch_min_css()`
/// today. TODO(page-panel-touch): fold the form's buttons into the posture's
/// touch minimum rather than relying on the ambient font scale.
pub(super) fn button_css(active: bool) -> String {
    format!(
        "padding: 2px 6px; border-radius: 3px; border: 1px solid {border}; \
         cursor: pointer; font-size: {fs}px; background: {bg}; color: {fg};",
        border = if active {
            tokens::COLOR_TAB_ACTIVE_INDICATOR
        } else {
            tokens::COLOR_BORDER_CHROME
        },
        fs = tokens::FONT_SIZE_LABEL,
        bg = if active {
            tokens::COLOR_SURFACE_3
        } else {
            tokens::COLOR_SURFACE_2
        },
        fg = tokens::COLOR_TEXT_ON_CHROME,
    )
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
            style: button_css(active),
            onclick: move |_| {
                let Some(current) = page_edit_target(&ds, &name) else {
                    return;
                };
                let next = apply_preset(&current, preset);
                let guard = sync.loro_doc.read();
                let Some(ldoc) = guard.as_ref() else { return };
                if set_page_style_geometry(ldoc, &name, &next).is_ok() {
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

/// A labelled row of controls.
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

/// The "apply this page style to the section the caret is in" button.
///
/// Rendered only when there **is** a caret and its section does not already use
/// this style, so the control is never a no-op that looks like an action.
fn apply_here_button(
    doc_state: &Arc<Mutex<DocumentState>>,
    name: String,
    sync: StyleEditorSync,
) -> Element {
    let focus = sync.cursor_state.read().focus.clone();
    let Some(section) = caret_section_index(doc_state, focus.as_ref()) else {
        return rsx! {};
    };
    let already = doc_state
        .lock()
        .ok()
        .and_then(|s| {
            let doc = s.document.as_ref()?;
            Some(doc.sections.get(section)?.page_style.as_ref()?.as_str() == name)
        })
        .unwrap_or(false);
    if already {
        return rsx! {};
    }
    let ds = Arc::clone(doc_state);
    rsx! {
        button {
            style: button_css(false),
            onclick: move |_| {
                let guard = sync.loro_doc.read();
                let Some(ldoc) = guard.as_ref() else { return };
                if set_section_page_style(ldoc, section, &name).is_ok() {
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
            { fl!("style-page-apply-here") }
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
    // A user-defined size goes through the same geometry mutation as a preset;
    // only the way the size was chosen differs.
    let ds_size = Arc::clone(doc_state);
    let size_name = name.clone();
    let on_custom_size = move |size: PageSize| {
        let Some(mut next) = page_edit_target(&ds_size, &size_name) else {
            return;
        };
        // T6.3: remember a size the catalogue cannot name, so the picker can
        // offer it again. Recorded before the mutation rather than after, so a
        // size the user typed is kept even if the document write fails.
        super::super::editor_defaults::remember_custom_size(&size);
        next.page_size = size;
        let guard = sync.loro_doc.read();
        let Some(ldoc) = guard.as_ref() else { return };
        if set_page_style_geometry(ldoc, &size_name, &next).is_ok() {
            apply_mutation_and_relayout(&ds_size, ldoc);
            drop(guard);
            post_mutation_sync(
                &ds_size,
                sync.loro_doc,
                sync.cursor_state,
                sync.undo_manager,
                sync.can_undo,
                sync.can_redo,
            );
        }
    };
    let unit = page_measurement_unit();
    let count = column_count(&layout);
    rsx! {
        div {
            style: format!("display: flex; flex-direction: column; padding: {}px;", tokens::SPACE_2),

            PageRenameField { key: "{name}", name: name.clone(), on_rename }

            { preset_row(fl!("style-page-orientation"), rsx! {
                { btn(fl!("ribbon-orientation-portrait-aria"), PagePreset::Portrait) }
                { btn(fl!("ribbon-orientation-landscape-aria"), PagePreset::Landscape) }
            }) }
            { size_section(&layout, unit, &btn, on_custom_size) }
            { preset_row(fl!("style-page-margins"), rsx! {
                { btn(fl!("ribbon-margin-normal-aria"), PagePreset::MarginsNormal) }
                { btn(fl!("ribbon-margin-narrow-aria"), PagePreset::MarginsNarrow) }
                { btn(fl!("ribbon-margin-wide-aria"), PagePreset::MarginsWide) }
            }) }
            { preset_row(fl!("style-page-columns"), rsx! {
                { btn(fl!("ribbon-columns-one-aria"), PagePreset::Columns(1)) }
                { btn(fl!("ribbon-columns-two-aria"), PagePreset::Columns(2)) }
                { btn(fl!("ribbon-columns-three-aria"), PagePreset::Columns(3)) }
                { btn(fl!("style-page-columns-fewer"), PagePreset::ColumnCountDelta(-1)) }
                span {
                    style: format!(
                        "font-size: {fs}px; color: {fg}; min-width: 16px; text-align: center;",
                        fs = tokens::FONT_SIZE_LABEL,
                        fg = tokens::COLOR_TEXT_ON_CHROME,
                    ),
                    "{count}"
                }
                { btn(fl!("style-page-columns-more"), PagePreset::ColumnCountDelta(1)) }
                { btn(fl!("style-page-column-separator"), PagePreset::ToggleSeparator) }
            }) }
            { preset_row(fl!("style-page-apply-label"), apply_here_button(doc_state, name.clone(), sync)) }
            { unit_row(unit, sync.settings_generation) }
            { new_document_defaults_row(&layout, sync.settings_generation) }
        }
    }
}
