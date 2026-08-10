// SPDX-License-Identifier: Apache-2.0

//! The ribbon entry points for the tabbed dialogs (design sections 2, 4, 5, 6).
//!
//! # Why these are labelled, and labelled with an ellipsis
//!
//! Each button here **opens a dialog** rather than performing the action, and
//! the trailing ellipsis is the long-standing convention that says so before
//! the click. They are text rather than icons because the icon set has no
//! glyph that distinguishes "insert a table" from "insert a table, but ask me
//! first" — an icon that lied about which of the two you were pressing would be
//! worse than a word that does not.
//!
//! # These sit *beside* the existing quick actions, not in place of them
//!
//! The Insert tab already inserts a 2×2 table and opens a bare URL panel in one
//! click, and those remain: a dialog is the configured path, not the only path.
//! Retiring the quick actions is a product decision, not an implementation
//! detail of adding the dialogs, so this change does not make it.

use appthere_ui::{AtRibbonIconButton, RibbonGroupSpec, estimate_group_metrics, tokens};
use dioxus::prelude::*;
use loki_i18n::fl;

use super::link_dialog::LinkDraft;
use super::table_dialog::TableSpec;

/// A compact text label inside a ribbon button, for actions with no icon.
pub(super) fn dialog_label(text: &str) -> Element {
    rsx! {
        span {
            style: format!(
                "font-size: {fs}px; color: inherit; white-space: nowrap;",
                fs = tokens::FONT_SIZE_LABEL,
            ),
            "{text}"
        }
    }
}

/// The Format tab's **Character** group — opens the span formatting dialog.
pub(super) fn character_group(mut open: Signal<bool>, priority: u8) -> RibbonGroupSpec {
    RibbonGroupSpec {
        metrics: estimate_group_metrics(priority, 1, true),
        label: Some(fl!("ribbon-group-character")),
        aria_label: fl!("ribbon-group-character"),
        content: rsx! {
            AtRibbonIconButton {
                aria_label: fl!("ribbon-character-dialog-aria"),
                is_active: open(),
                is_disabled: false,
                on_click: move |_| {
                    let is_open = open();
                    open.set(!is_open);
                },
                {dialog_label(&fl!("ribbon-character-dialog-label"))}
            }
        },
    }
}

/// The Insert tab's **Link…** button — opens the Insert link dialog.
///
/// Opens on a fresh [`LinkDraft`]: the dialog seeds nothing from the selection
/// yet, and a draft carried over from a previous open would put a stale address
/// in front of a new selection.
pub(super) fn link_button(mut open: Signal<Option<LinkDraft>>) -> Element {
    rsx! {
        AtRibbonIconButton {
            aria_label: fl!("ribbon-insert-link-dialog-aria"),
            is_active: open.read().is_some(),
            is_disabled: false,
            on_click: move |_| {
                if open.read().is_some() {
                    open.set(None);
                } else {
                    open.set(Some(LinkDraft::default()));
                }
            },
            {dialog_label(&fl!("ribbon-insert-link-dialog-label"))}
        }
    }
}

/// The Insert tab's **Table…** button — opens the Insert table dialog.
pub(super) fn table_button(mut open: Signal<Option<TableSpec>>) -> Element {
    rsx! {
        AtRibbonIconButton {
            aria_label: fl!("ribbon-insert-table-dialog-aria"),
            is_active: open.read().is_some(),
            is_disabled: false,
            on_click: move |_| {
                if open.read().is_some() {
                    open.set(None);
                } else {
                    open.set(Some(TableSpec::default()));
                }
            },
            {dialog_label(&fl!("ribbon-insert-table-dialog-label"))}
        }
    }
}

/// The Publish tab's **Properties…** button — opens the metadata dialog.
pub(super) fn properties_button(mut open: Signal<bool>) -> Element {
    rsx! {
        AtRibbonIconButton {
            aria_label: fl!("ribbon-properties-dialog-aria"),
            is_active: open(),
            is_disabled: false,
            on_click: move |_| {
                let is_open = open();
                open.set(!is_open);
            },
            {dialog_label(&fl!("ribbon-properties-dialog-label"))}
        }
    }
}
