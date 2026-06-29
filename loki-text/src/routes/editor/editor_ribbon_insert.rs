// SPDX-License-Identifier: Apache-2.0

//! Insert tab ribbon content for the document editor (Spec 04 M4).
//!
//! [`insert_tab_content`] returns the `Element` passed to
//! [`AtRibbon::tab_content`] when the Insert tab is active. It hosts the
//! create controls for objects with a native Loro mapping. The first control
//! is **Link**, which opens the URL panel ([`super::editor_insert_panel`]);
//! further controls (image, table, footnote) arrive in later increments as
//! their create paths land — no control is shown before it does something.

use appthere_ui::{AtIcon, AtRibbonGroup, AtRibbonIconButton, LUCIDE_LINK};
use dioxus::prelude::*;
use loki_i18n::fl;

/// Builds the Insert tab ribbon content element.
///
/// `link_draft` is the shared panel-state signal: setting it to `Some("")`
/// opens the hyperlink URL panel with an empty field.
pub(super) fn insert_tab_content(mut link_draft: Signal<Option<String>>) -> Element {
    rsx! {
        // ── Links group ───────────────────────────────────────────────────────
        AtRibbonGroup {
            label:      Some(fl!("ribbon-group-insert")),
            aria_label: fl!("ribbon-group-insert"),

            AtRibbonIconButton {
                aria_label:  fl!("ribbon-insert-link-aria"),
                is_active:   link_draft.read().is_some(),
                is_disabled: false,
                on_click: move |_| {
                    if link_draft.read().is_some() {
                        link_draft.set(None);
                    } else {
                        link_draft.set(Some(String::new()));
                    }
                },
                AtIcon { path_d: LUCIDE_LINK.to_string() }
            }
        }
    }
}
