// SPDX-License-Identifier: Apache-2.0

//! Home screen route component.
//!
//! A thin wrapper over [`appthere_ui::AtHomeTab`] that wires Loki Text's
//! static template data, the platform file picker, and the Dioxus router
//! to the generic component's props and callbacks.
//!
//! All user-visible strings are defined as named constants so they are
//! ready for future `loki_i18n` / Fluent integration.

use appthere_ui::{AtHomeTab, BuiltinTemplate, RecentDocument};
use dioxus::prelude::*;
use loki_file_access::{FilePicker, PickOptions};

use crate::routes::Route;
use crate::tabs::OpenTab;
use crate::utils::display_title_from_path;

// ── String constants (i18n-ready) ─────────────────────────────────────────────

const APP_NAME: &str = "Loki Text";
const TEMPLATES_LABEL: &str = "Templates";
const RECENT_LABEL: &str = "Recent";
const BROWSE_LABEL: &str = "Browse\u{2026}";
const OPEN_FILE_LABEL: &str = "Open File\u{2026}";
const EMPTY_RECENT_LABEL: &str = "No recent documents. Open a file to get started.";

// ── Static template data ──────────────────────────────────────────────────────

const TEMPLATES: &[BuiltinTemplate] = &[
    BuiltinTemplate {
        name: "Blank",
        description: "Empty document",
        format_label: "DOCX",
    },
    BuiltinTemplate {
        name: "Letter",
        description: "Formal letter template",
        format_label: "DOCX",
    },
    BuiltinTemplate {
        name: "Report",
        description: "Multi-section report",
        format_label: "DOCX",
    },
    BuiltinTemplate {
        name: "Resume",
        description: "Single-page r\u{00E9}sum\u{00E9}",
        format_label: "DOCX",
    },
    BuiltinTemplate {
        name: "Invoice",
        description: "Simple invoice template",
        format_label: "DOCX",
    },
];

// ── Static recent-file placeholder data ───────────────────────────────────────
//
// TODO(recent-files): Replace with persisted MRU list from loki_file_access or
// a future loki_prefs crate.

fn placeholder_recent_documents() -> Vec<RecentDocument> {
    vec![
        RecentDocument {
            title: "Q1 Report".to_string(),
            path: "~/Documents/Work/2026/\u{2026}".to_string(),
            modified_at: "2026-04-12  14:30".to_string(),
        },
        RecentDocument {
            title: "Meeting Notes".to_string(),
            path: "~/Documents/Meetings/\u{2026}".to_string(),
            modified_at: "2026-04-11  09:15".to_string(),
        },
        RecentDocument {
            title: "Budget Draft".to_string(),
            path: "~/Documents/Finance/\u{2026}".to_string(),
            modified_at: "2026-04-09  16:45".to_string(),
        },
    ]
}

// ── MIME types accepted by the file picker ────────────────────────────────────

const MIME_TYPES: &[&str] = &[
    "application/vnd.openxmlformats-officedocument.wordprocessingml.document",
    "application/vnd.oasis.opendocument.text",
];

// ── Helpers ───────────────────────────────────────────────────────────────────

/// Push `path` as a new open tab, or switch to its existing tab if already open.
///
/// Returns the new active tab-bar index (1-based; 0 is the Home tab).
fn push_or_switch_tab(mut tabs: Signal<Vec<OpenTab>>, mut active_tab: Signal<usize>, path: String) {
    let title = display_title_from_path(&path);
    let existing = tabs.read().iter().position(|t| t.path == path);
    if let Some(idx) = existing {
        // Tab already open — switch to it (tab-bar index = Vec index + 1).
        *active_tab.write() = idx + 1;
    } else {
        tabs.write().push(OpenTab {
            title,
            path,
            is_dirty: false,
            is_discarded: false,
        });
        // TODO(tabs): Replace router-driven navigation with tab-driven navigation
        // — the active tab index should determine which Editor route is displayed,
        // not vice versa.
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

    // Holds the last file-picker error message, if any.
    // AtHomeTab surfaces the on_open_file callback; error display is handled
    // by the caller in a future pass when AtHomeTab gains an error prop.
    let pick_error: Signal<Option<String>> = use_signal(|| None);

    let on_open_file = move |_| {
        let nav = navigator;
        let mut err_sig = pick_error;
        let tabs = tabs;
        let active_tab = active_tab;
        spawn(async move {
            let picker = FilePicker::new();
            let opts = PickOptions {
                mime_types: MIME_TYPES.iter().map(|s| (*s).to_string()).collect(),
                filter_label: Some("Documents".to_string()),
                multi: false,
            };
            match picker.pick_file_to_open(opts).await {
                Ok(Some(token)) => {
                    let path = token.serialize();
                    push_or_switch_tab(tabs, active_tab, path.clone());
                    nav.push(Route::Editor { path });
                }
                Ok(None) => { /* user cancelled — no-op */ }
                Err(e) => {
                    *err_sig.write() = Some(e.to_string());
                }
            }
        });
    };

    rsx! {
        AtHomeTab {
            app_name:            APP_NAME,
            templates:           TEMPLATES.to_vec(),
            recent_documents:    placeholder_recent_documents(),
            templates_label:     TEMPLATES_LABEL,
            recent_label:        RECENT_LABEL,
            browse_label:        BROWSE_LABEL,
            open_file_label:     OPEN_FILE_LABEL,
            empty_recent_label:  EMPTY_RECENT_LABEL,
            // TODO(templates): navigate to the editor with the chosen template applied.
            on_template_select:  |_idx| {},
            // TODO(browse-templates): open a template browser dialog.
            on_browse_templates: |_| {},
            // TODO(recent-files): open the selected recent document via its stored token.
            on_recent_open:      |_idx| {},
            on_open_file:        on_open_file,
        }
    }
}
