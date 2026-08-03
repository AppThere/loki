// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 AppThere Loki contributors

//! File-dialog flows for the Home route, extracted from `home.rs` for the
//! 300-line ceiling.

use dioxus::prelude::*;
use dioxus_router::Navigator;
use loki_file_access::{FilePicker, PickOptions, PickerError};
use loki_i18n::fl;

use super::templates::TEMPLATE_MIME_TYPES;
use crate::new_document::new_import_tab;
use crate::routes::Route;
use crate::routes::home_util::push_new_tab;
use crate::tabs::OpenTab;
use crate::utils::display_title_from_path;

/// The **Browse templates** card: opens the system file dialog filtered to
/// template types (`.dotx` / `.dotm` / `.ott`).
///
/// It used to raise an in-app overlay listing the same six bundled templates
/// the gallery already shows beside it, so the card led back to where the user
/// started and there was no route to a template of their own.
///
/// The chosen file is opened the way `home.rs`'s open-file path opens a
/// template: a fresh, **detached** document, so saving prompts Save As rather
/// than writing over the template, and it is kept out of recents.
pub(super) fn browse_templates(
    nav: Navigator,
    mut err_sig: Signal<Option<String>>,
    tabs: Signal<Vec<OpenTab>>,
    active_tab: Signal<usize>,
) {
    spawn(async move {
        let picker = FilePicker::new();
        let opts = PickOptions {
            mime_types: TEMPLATE_MIME_TYPES
                .iter()
                .map(|s| (*s).to_string())
                .collect(),
            filter_label: Some(fl!("home-template-filter-label")),
            multi: false,
        };
        match picker.pick_file_to_open(opts).await {
            Ok(Some(token)) => {
                let serialized = token.serialize();
                let title = display_title_from_path(&serialized);
                let path = push_new_tab(tabs, active_tab, new_import_tab(&serialized, title));
                nav.push(Route::Editor { path });
            }
            Ok(None) => { /* user cancelled — no-op */ }
            Err(PickerError::Platform { .. }) => {
                *err_sig.write() = Some(fl!("error-picker-not-supported"));
            }
            Err(e) => {
                *err_sig.write() = Some(e.to_string());
            }
        }
    });
}
