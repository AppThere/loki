// SPDX-License-Identifier: Apache-2.0

//! Home screen route component.
//!
//! A thin wrapper over [`appthere_ui::AtHomeTab`] that wires Loki Text's
//! template data, the platform file picker, and the Dioxus router to the
//! generic component's props and callbacks.
//!
//! All user-visible strings come from `loki_i18n::fl!()` — no hardcoded
//! English literals.

use appthere_ui::{AtHomeTab, BuiltinTemplate, RecentDocument};
use dioxus::prelude::*;
use loki_file_access::{FilePicker, PickOptions, PickerError};
use loki_i18n::fl;

use crate::new_document::new_blank_tab;
use crate::recent_documents::RecentDocuments;
use crate::routes::Route;
use crate::tabs::OpenTab;
use crate::utils::display_title_from_path;

// ── MIME types accepted by the file picker ────────────────────────────────────

const MIME_TYPES: &[&str] = &[
    "application/vnd.openxmlformats-officedocument.wordprocessingml.document",
    "application/vnd.oasis.opendocument.text",
];

// ── Template data ─────────────────────────────────────────────────────────────

fn make_templates() -> Vec<BuiltinTemplate> {
    vec![
        BuiltinTemplate {
            name: fl!("home-template-blank"),
            description: fl!("home-template-blank-description"),
            format_label: fl!("home-template-blank-format"),
        },
        BuiltinTemplate {
            name: fl!("home-template-letter"),
            description: fl!("home-template-letter-description"),
            format_label: fl!("home-template-letter-format"),
        },
        BuiltinTemplate {
            name: fl!("home-template-report"),
            description: fl!("home-template-report-description"),
            format_label: fl!("home-template-report-format"),
        },
        BuiltinTemplate {
            name: fl!("home-template-resume"),
            description: fl!("home-template-resume-description"),
            format_label: fl!("home-template-resume-format"),
        },
        BuiltinTemplate {
            name: fl!("home-template-invoice"),
            description: fl!("home-template-invoice-description"),
            format_label: fl!("home-template-invoice-format"),
        },
    ]
}

// ── Helpers ───────────────────────────────────────────────────────────────────

/// Push `path` as a new open tab, or switch to its existing tab if already open.
fn push_or_switch_tab(mut tabs: Signal<Vec<OpenTab>>, mut active_tab: Signal<usize>, path: String) {
    let title = display_title_from_path(&path);
    let existing = tabs.read().iter().position(|t| t.path == path);
    if let Some(idx) = existing {
        *active_tab.write() = idx + 1;
    } else {
        tabs.write().push(OpenTab {
            title,
            path,
            is_dirty: false,
            is_discarded: false,
        });
        // TODO(tabs): Replace router-driven navigation with tab-driven navigation.
        *active_tab.write() = tabs.read().len(); // new tab is last; +1 for Home
    }
}

// ── Home ──────────────────────────────────────────────────────────────────────

/// Home screen component — wraps [`AtHomeTab`] with Loki Text data and routing.
///
/// Document picking and navigation stay here; all layout and presentation
/// are delegated to `AtHomeTab`.
#[component]
pub fn Home() -> Element {
    let navigator = use_navigator();

    let tabs = use_context::<Signal<Vec<OpenTab>>>();
    let active_tab = use_context::<Signal<usize>>();
    let mut recent_docs = use_context::<Signal<RecentDocuments>>();

    // Holds the last file-picker error message, if any.
    let pick_error: Signal<Option<String>> = use_signal(|| None);

    // ── on_template_select ────────────────────────────────────────────────────
    //
    // Index 0 = "Blank" — opens a new blank document.
    // All other indices are deferred (templates not yet implemented).
    let on_template_select = move |idx: usize| {
        if idx == 0 {
            let tab = new_blank_tab();
            let path = tab.path.clone();
            let nav = navigator;
            let mut t = tabs;
            let mut a = active_tab;
            t.write().push(tab);
            *a.write() = t.read().len(); // new tab is last; +1 for Home
            nav.push(Route::Editor { path });
        }
        // TODO(templates): Apply the selected built-in template (idx > 0).
    };

    // ── on_open_file ──────────────────────────────────────────────────────────
    let on_open_file = move |_| {
        let nav = navigator;
        let mut err_sig = pick_error;
        let tabs = tabs;
        let active_tab = active_tab;
        let mut recent = recent_docs;
        spawn(async move {
            let picker = FilePicker::new();
            let opts = PickOptions {
                mime_types: MIME_TYPES.iter().map(|s| (*s).to_string()).collect(),
                filter_label: Some(fl!("home-filter-label")),
                multi: false,
            };
            match picker.pick_file_to_open(opts).await {
                Ok(Some(token)) => {
                    let path = token.serialize();
                    let title = display_title_from_path(&path);
                    push_or_switch_tab(tabs, active_tab, path.clone());
                    recent.write().record(path.clone(), title);
                    recent.read().save();
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
    };

    // ── on_recent_open ────────────────────────────────────────────────────────
    let on_recent_open = move |idx: usize| {
        let nav = navigator;
        let entry = recent_docs.read().entries.get(idx).cloned();
        if let Some(entry) = entry {
            push_or_switch_tab(tabs, active_tab, entry.path.clone());
            recent_docs
                .write()
                .record(entry.path.clone(), entry.title.clone());
            recent_docs.read().save();
            nav.push(Route::Editor { path: entry.path });
        }
    };

    // ── Map RecentEntry → RecentDocument (appthere-ui type) ──────────────────
    let recent_list: Vec<RecentDocument> = recent_docs
        .read()
        .entries
        .iter()
        .map(|e| RecentDocument {
            title: e.title.clone(),
            path: e.path.clone(),
            modified_at: e.modified_at.clone(),
        })
        .collect();

    rsx! {
        AtHomeTab {
            app_name:            fl!("shell-app-name"),
            templates:           make_templates(),
            recent_documents:    recent_list,
            templates_label:     fl!("home-templates-heading"),
            recent_label:        fl!("home-recent-heading"),
            // Empty string hides the Browse card until template browsing is implemented.
            browse_label:        String::new(),
            open_file_label:     fl!("home-open-file"),
            empty_recent_label:  fl!("home-no-recent"),
            pick_error:          pick_error,
            on_template_select:  on_template_select,
            // TODO(browse-templates): open a template browser dialog.
            on_browse_templates: |_| {},
            on_recent_open:      on_recent_open,
            on_open_file:        on_open_file,
        }
    }
}
