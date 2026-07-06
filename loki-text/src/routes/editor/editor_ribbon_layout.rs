// SPDX-License-Identifier: Apache-2.0

//! Layout ribbon tab content (Spec 04 M5, plan 4a.2).
//!
//! Layout controls change the section page geometry in the CRDT and relayout,
//! so the document immediately re-flows. Controls: page **orientation**
//! (portrait/landscape), **margin** presets, page **size** (A4/Letter), and
//! **columns** (one/two/three).

use std::sync::{Arc, Mutex};

use appthere_ui::{
    AT_COLUMNS_ONE, AT_COLUMNS_THREE, AT_COLUMNS_TWO, AT_MARGIN_NARROW, AT_MARGIN_NORMAL,
    AT_MARGIN_WIDE, AT_PAGE_A4, AT_PAGE_LANDSCAPE, AT_PAGE_LETTER, AT_PAGE_PORTRAIT, AtIcon,
    AtRibbonGroups, AtRibbonIconButton, RibbonGroupSpec, estimate_group_metrics,
};
use dioxus::prelude::*;
use loki_doc_model::{
    MutationError, document_column_count, document_is_landscape, document_margins,
    document_page_size, set_document_columns, set_document_margins, set_document_orientation,
    set_document_page_size,
};
use loki_i18n::fl;

use super::editor_keydown_ctrl::post_mutation_sync;
use crate::editing::cursor::CursorState;
use crate::editing::state::{DocumentState, apply_mutation_and_relayout};

/// A margin preset: `(aria-key, top, bottom, left, right, icon)` — points.
const MARGIN_PRESETS: &[(&str, f64, f64, f64, f64, &str)] = &[
    (
        "ribbon-margin-normal-aria",
        72.0,
        72.0,
        72.0,
        72.0,
        AT_MARGIN_NORMAL,
    ),
    (
        "ribbon-margin-narrow-aria",
        36.0,
        36.0,
        36.0,
        36.0,
        AT_MARGIN_NARROW,
    ),
    (
        "ribbon-margin-wide-aria",
        72.0,
        72.0,
        144.0,
        144.0,
        AT_MARGIN_WIDE,
    ),
];

/// Whether the document's `current` margins match a preset `(top, bottom, left,
/// right)` within half a point — drives which preset button shows active.
fn margin_matches(current: Option<(f64, f64, f64, f64)>, preset: (f64, f64, f64, f64)) -> bool {
    let Some((t, b, l, r)) = current else {
        return false;
    };
    let close = |a: f64, x: f64| (a - x).abs() < 0.5;
    close(t, preset.0) && close(b, preset.1) && close(l, preset.2) && close(r, preset.3)
}

/// A page-size preset: `(aria-key, portrait_width, portrait_height, icon)` — pt.
const PAGE_SIZE_PRESETS: &[(&str, f64, f64, &str)] = &[
    ("ribbon-page-a4-aria", 595.28, 841.89, AT_PAGE_A4),
    ("ribbon-page-letter-aria", 612.0, 792.0, AT_PAGE_LETTER),
];

/// A column preset: `(aria-key, count, icon)`.
const COLUMN_PRESETS: &[(&str, u8, &str)] = &[
    ("ribbon-columns-one-aria", 1, AT_COLUMNS_ONE),
    ("ribbon-columns-two-aria", 2, AT_COLUMNS_TWO),
    ("ribbon-columns-three-aria", 3, AT_COLUMNS_THREE),
];

/// Whether the document's `current` page size is the `preset` paper, comparing
/// the orientation-independent short/long edges within a point.
fn page_size_matches(current: Option<(f64, f64)>, preset: (f64, f64)) -> bool {
    let Some((w, h)) = current else {
        return false;
    };
    let (cmin, cmax) = (w.min(h), w.max(h));
    let (pmin, pmax) = (preset.0.min(preset.1), preset.0.max(preset.1));
    (cmin - pmin).abs() < 1.0 && (cmax - pmax).abs() < 1.0
}

/// Runs a page-geometry mutation `f` against the live document, relays out, and
/// syncs undo/redo — the shared path for every Layout button.
fn apply_and_sync(
    doc_state: &Arc<Mutex<DocumentState>>,
    loro_doc: Signal<Option<loro::LoroDoc>>,
    cursor_state: Signal<CursorState>,
    undo_manager: Signal<Option<loro::UndoManager>>,
    can_undo: Signal<bool>,
    can_redo: Signal<bool>,
    f: impl FnOnce(&loro::LoroDoc) -> Result<(), MutationError>,
) {
    {
        let guard = loro_doc.read();
        let Some(ldoc) = guard.as_ref() else {
            return;
        };
        if f(ldoc).is_err() {
            return;
        }
        apply_mutation_and_relayout(doc_state, ldoc);
    }
    post_mutation_sync(
        doc_state,
        loro_doc,
        cursor_state,
        undo_manager,
        can_undo,
        can_redo,
    );
}

/// Builds the Layout tab content (Orientation + Margins groups).
#[allow(clippy::too_many_arguments)]
pub(super) fn layout_tab_content(
    doc_state: &Arc<Mutex<DocumentState>>,
    loro_doc: Signal<Option<loro::LoroDoc>>,
    cursor_state: Signal<CursorState>,
    undo_manager: Signal<Option<loro::UndoManager>>,
    can_undo: Signal<bool>,
    can_redo: Signal<bool>,
) -> Element {
    let (landscape, margins, page_size, columns) = loro_doc
        .read()
        .as_ref()
        .map(|ldoc| {
            (
                document_is_landscape(ldoc),
                document_margins(ldoc),
                document_page_size(ldoc),
                document_column_count(ldoc),
            )
        })
        .unwrap_or((false, None, None, 1));
    let ds_portrait = Arc::clone(doc_state);
    let ds_landscape = Arc::clone(doc_state);

    // The four Layout groups, declared as collapse specs (Spec 04 M3). Priority
    // descends left→right so Columns overflows first and Orientation stays full
    // the longest; the container runs the width-driven cascade + overflow menu.
    let orientation = RibbonGroupSpec {
        metrics: estimate_group_metrics(3, 2, true),
        label: Some(fl!("ribbon-group-orientation")),
        aria_label: fl!("ribbon-group-orientation"),
        content: rsx! {
            AtRibbonIconButton {
                aria_label:  fl!("ribbon-orientation-portrait-aria"),
                is_active:   !landscape,
                is_disabled: false,
                on_click: move |_| apply_and_sync(
                    &ds_portrait, loro_doc, cursor_state, undo_manager,
                    can_undo, can_redo, |l| set_document_orientation(l, false),
                ),
                AtIcon { path_d: AT_PAGE_PORTRAIT.to_string() }
            }
            AtRibbonIconButton {
                aria_label:  fl!("ribbon-orientation-landscape-aria"),
                is_active:   landscape,
                is_disabled: false,
                on_click: move |_| apply_and_sync(
                    &ds_landscape, loro_doc, cursor_state, undo_manager,
                    can_undo, can_redo, |l| set_document_orientation(l, true),
                ),
                AtIcon { path_d: AT_PAGE_LANDSCAPE.to_string() }
            }
        },
    };

    let margins_group = RibbonGroupSpec {
        metrics: estimate_group_metrics(2, MARGIN_PRESETS.len(), true),
        label: Some(fl!("ribbon-group-margins")),
        aria_label: fl!("ribbon-group-margins"),
        content: rsx! {
            for (aria, t, b, l, r, icon) in MARGIN_PRESETS.iter().copied() {
                AtRibbonIconButton {
                    key: "{aria}",
                    aria_label:  fl!(aria),
                    is_active:   margin_matches(margins, (t, b, l, r)),
                    is_disabled: false,
                    on_click: {
                        let ds = Arc::clone(doc_state);
                        move |_| apply_and_sync(
                            &ds, loro_doc, cursor_state, undo_manager,
                            can_undo, can_redo, |lo| set_document_margins(lo, t, b, l, r),
                        )
                    },
                    AtIcon { path_d: icon.to_string() }
                }
            }
        },
    };

    let size_group = RibbonGroupSpec {
        metrics: estimate_group_metrics(1, PAGE_SIZE_PRESETS.len(), true),
        label: Some(fl!("ribbon-group-page-size")),
        aria_label: fl!("ribbon-group-page-size"),
        content: rsx! {
            for (aria, pw, ph, icon) in PAGE_SIZE_PRESETS.iter().copied() {
                AtRibbonIconButton {
                    key: "{aria}",
                    aria_label:  fl!(aria),
                    is_active:   page_size_matches(page_size, (pw, ph)),
                    is_disabled: false,
                    on_click: {
                        let ds = Arc::clone(doc_state);
                        move |_| apply_and_sync(
                            &ds, loro_doc, cursor_state, undo_manager,
                            can_undo, can_redo, |lo| set_document_page_size(lo, pw, ph),
                        )
                    },
                    AtIcon { path_d: icon.to_string() }
                }
            }
        },
    };

    let columns_group = RibbonGroupSpec {
        metrics: estimate_group_metrics(0, COLUMN_PRESETS.len(), true),
        label: Some(fl!("ribbon-group-columns")),
        aria_label: fl!("ribbon-group-columns"),
        content: rsx! {
            for (aria, count, icon) in COLUMN_PRESETS.iter().copied() {
                AtRibbonIconButton {
                    key: "{aria}",
                    aria_label:  fl!(aria),
                    is_active:   columns == count,
                    is_disabled: false,
                    on_click: {
                        let ds = Arc::clone(doc_state);
                        move |_| apply_and_sync(
                            &ds, loro_doc, cursor_state, undo_manager,
                            can_undo, can_redo, |lo| set_document_columns(lo, count),
                        )
                    },
                    AtIcon { path_d: icon.to_string() }
                }
            }
        },
    };

    rsx! {
        AtRibbonGroups {
            groups: vec![orientation, margins_group, size_group, columns_group],
            overflow_aria_label: fl!("ribbon-overflow-aria"),
        }
    }
}

#[cfg(test)]
#[path = "editor_ribbon_layout_tests.rs"]
mod tests;
