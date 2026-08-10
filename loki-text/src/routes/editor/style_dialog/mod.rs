// SPDX-License-Identifier: Apache-2.0

//! The paragraph style editor dialog (Spec 05 M2 / M6, design section 1).
//!
//! A tabbed modal over the paragraph family. **Every applicable property is
//! shown whether or not it is set locally** — the fix for the old panel's
//! local-only blindness (audit SM-3) — and each carries its provenance on the
//! line beneath it.
//!
//! # Structure
//!
//! | module | role |
//! | --- | --- |
//! | [`tabs`] | the tab set and its collapse priority |
//! | [`draft`] | the edit buffer, with local-vs-inherited preserved |
//! | [`rows`] | resolving one property's provenance and phrasing it |
//! | [`borders`] | edge presets for the Borders tab |
//! | [`font_list`] | the bundled-first picker list |
//! | [`body`] | tab dispatch and the preview rail |
//! | `tab_*` | one module per tab body |
//!
//! The chrome — modal frame, responsive posture, tab strip with its `More ▾`
//! collapse and Compact section picker — is [`appthere_ui`]'s
//! [`AtDialogShell`](appthere_ui::AtDialogShell) and
//! [`AtDialogTabStrip`](appthere_ui::AtDialogTabStrip), so the other six
//! dialogs inherit it unchanged.
//!
//! # Mounting
//!
//! A `#[component]` mounted at the boundary (ADR-0013) so it owns a hook scope
//! and reads the breakpoint itself, with no `compact` flag threaded from the
//! parent:
//!
//! ```rust,ignore
//! {open().then(|| rsx! { ParagraphStyleDialog { .. } })}
//! ```
//!
//! Its host must be `position: relative` and span the area to dim — the
//! [`AtDialogShell`](appthere_ui::AtDialogShell) mounting contract.

mod body;
mod borders;
mod draft;
mod fields;
mod font_list;
mod font_picker;
mod preview;
mod rows;
mod tab_alignment;
mod tab_borders;
mod tab_borders_fields;
mod tab_flow;
mod tab_font;
mod tab_general;
mod tab_indents;
mod tab_stops;
mod tab_stops_cells;
mod tab_stops_edit;
mod tabs;

use std::rc::Rc;
use std::sync::{Arc, Mutex};

use appthere_ui::responsive::use_breakpoint;
use appthere_ui::tokens;
use appthere_ui::{AtDialogButton, AtDialogShell, AtDialogTabStrip, DialogPosture, DialogWidth};
use dioxus::prelude::*;
use loki_doc_model::style::{StyleCatalog, StyleId};
use loki_i18n::fl;

use super::editor_keydown_ctrl::post_mutation_sync;
use super::editor_style_catalog::{catalog_snapshot, commit_style_to_loro, get_catalog_style};
use super::editor_style_editor::StyleEditorSync;
use crate::editing::state::{DocumentState, apply_mutation_and_relayout};
use draft::ParaDialogDraft;
use tabs::ParaTab;

/// Props for [`ParagraphStyleDialog`].
///
/// Hand-written `PartialEq` for the same reason `SpellPopoverProps` has one:
/// `Arc<Mutex<DocumentState>>` and `Rc<Vec<String>>` implement no `PartialEq`,
/// so they are compared by pointer — "the same handle", which is what a props
/// comparison wants. `#[component]` cannot derive that, so the component is a
/// PascalCase function taking the props struct (the house pattern).
#[derive(Clone, Props)]
pub(super) struct ParagraphStyleDialogProps {
    /// Shared document state, for reading the catalog and committing the style.
    pub(super) doc_state: Arc<Mutex<DocumentState>>,
    /// The style being edited; setting it to `None` closes the dialog, and
    /// setting it to another id re-targets the dialog — which is how the
    /// inherited rows' "Edit there" jump works.
    pub(super) open_style: Signal<Option<String>>,
    /// The id of the style to edit.
    pub(super) style_id: String,
    /// Font families enumerated on this device (memoised by the caller).
    pub(super) font_families: Rc<Vec<String>>,
    /// Loro / undo plumbing shared with the inline style panel.
    pub(super) sync: StyleEditorSync,
}

impl PartialEq for ParagraphStyleDialogProps {
    fn eq(&self, other: &Self) -> bool {
        Arc::ptr_eq(&self.doc_state, &other.doc_state)
            && self.open_style == other.open_style
            && self.style_id == other.style_id
            && Rc::ptr_eq(&self.font_families, &other.font_families)
            && self.sync == other.sync
    }
}

/// Renders the paragraph style editor for `style_id`.
// PascalCase for rsx; `#[component]` cannot be used — see the props docs.
#[allow(non_snake_case)]
pub(super) fn ParagraphStyleDialog(props: ParagraphStyleDialogProps) -> Element {
    let ParagraphStyleDialogProps {
        doc_state,
        mut open_style,
        style_id,
        font_families,
        sync,
    } = props;
    let id = StyleId::new(&style_id);
    let posture = DialogPosture::for_breakpoint(use_breakpoint());

    // The committed style seeds the draft; re-derived whenever the target
    // changes, so "Edit there" lands on the ancestor with its own values.
    let mut draft =
        use_signal(|| get_catalog_style(&doc_state, &style_id).map(ParaDialogDraft::new));
    let mut active_tab = use_signal(|| ParaTab::General);
    let menu_open = use_signal(|| false);
    let seeded_for = use_signal(|| style_id.clone());
    if *seeded_for.read() != style_id {
        let mut seeded_for = seeded_for;
        seeded_for.set(style_id.clone());
        draft.set(get_catalog_style(&doc_state, &style_id).map(ParaDialogDraft::new));
        active_tab.set(ParaTab::General);
    }

    // A style that is not in the catalog has nothing to edit — close rather
    // than render an empty shell over a style id that no longer resolves.
    let Some(current) = draft.read().clone() else {
        return rsx! {};
    };
    let catalog = catalog_snapshot(&doc_state).unwrap_or_default();
    let title = dialog_title(&catalog, &id);
    let tab = *active_tab.read();
    let dirty = current.is_dirty();

    let ds_apply = Arc::clone(&doc_state);
    let apply_id = style_id.clone();

    rsx! {
        AtDialogShell {
            title,
            subtitle: rsx! { { body::breadcrumb(&catalog, &id) } },
            close_aria_label: fl!("style-dialog-close-aria"),
            width: DialogWidth::Wide,
            on_close: move |_| open_style.set(None),

            tabs: rsx! {
                AtDialogTabStrip {
                    labels: ParaTab::labels(),
                    active: tab.index(),
                    inline_at_medium: ParaTab::INLINE_AT_MEDIUM,
                    menu_open,
                    more_label: fl!("style-dialog-tabs-more"),
                    on_select: move |idx: usize| active_tab.set(ParaTab::from_index(idx)),
                }
            },

            body: rsx! {
                { body::tab_body(tab, &catalog, &id, draft, open_style, posture, font_families) }
            },

            footer: rsx! {
                AtDialogButton {
                    label: fl!("style-dialog-reset-all"),
                    tertiary: true,
                    min_touch_px: posture.min_touch_px,
                    on_click: move |_| {
                        let mut next = draft.read().clone();
                        if let Some(d) = next.as_mut() {
                            d.reset_all_to_inherited();
                        }
                        draft.set(next);
                    },
                }
                div {
                    style: format!(
                        "display: flex; flex-direction: {dir}; gap: {gap}px;",
                        // Compact stacks the footer with the primary action
                        // first, so the committing button sits under the thumb.
                        dir = if posture.stack_footer { "column-reverse" } else { "row" },
                        gap = tokens::SPACE_2,
                    ),
                    AtDialogButton {
                        label: fl!("style-dialog-cancel"),
                        min_touch_px: posture.min_touch_px,
                        on_click: move |_| open_style.set(None),
                    }
                    AtDialogButton {
                        label: fl!("style-dialog-apply"),
                        primary: true,
                        // An untouched draft has nothing to commit, and a live
                        // Apply here would pin every inherited value locally.
                        disabled: !dirty,
                        min_touch_px: posture.min_touch_px,
                        on_click: move |_| {
                            let Some(d) = draft.read().clone() else { return };
                            if apply_style(&ds_apply, &sync, d.style.clone()) {
                                // Re-seed from the catalog so the provenance
                                // lines describe what was actually committed.
                                draft.set(
                                    get_catalog_style(&ds_apply, &apply_id).map(ParaDialogDraft::new),
                                );
                            }
                        },
                    }
                }
            },
        }
    }
}

/// `Paragraph style — Body indent`, falling back to the stable id when the
/// style has no display name.
fn dialog_title(catalog: &StyleCatalog, id: &StyleId) -> String {
    let name = catalog
        .paragraph_styles
        .get(id)
        .and_then(|s| s.display_name.clone())
        .unwrap_or_else(|| id.as_str().to_string());
    fl!("style-dialog-title", name = name)
}

/// Commits `style` through Loro and relayouts, returning whether it landed.
///
/// Persisting through the CRDT rather than mutating the catalog directly is what
/// makes the edit durable and undoable; the same path the inline panel's Apply
/// takes.
fn apply_style(
    doc_state: &Arc<Mutex<DocumentState>>,
    sync: &StyleEditorSync,
    style: loki_doc_model::style::ParagraphStyle,
) -> bool {
    let applied = {
        let guard = sync.loro_doc.read();
        if let Some(ldoc) = guard.as_ref() {
            commit_style_to_loro(ldoc, doc_state, style);
            apply_mutation_and_relayout(doc_state, ldoc);
            true
        } else {
            false
        }
    };
    if applied {
        post_mutation_sync(
            doc_state,
            sync.loro_doc,
            sync.cursor_state,
            sync.undo_manager,
            sync.can_undo,
            sync.can_redo,
        );
    }
    applied
}
