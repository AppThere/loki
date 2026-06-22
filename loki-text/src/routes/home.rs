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
use loki_file_access::{FileAccessToken, FilePicker, PickOptions, PickerError, SaveOptions};
use loki_i18n::fl;

use crate::new_document::{new_blank_tab, new_import_tab};
use crate::recent_documents::RecentDocuments;
use crate::routes::Route;
use crate::tabs::OpenTab;
use crate::utils::display_title_from_path;

// ── MIME types accepted by the file picker ────────────────────────────────────

const MIME_TYPES: &[&str] = &[
    "application/vnd.openxmlformats-officedocument.wordprocessingml.document",
    "application/vnd.oasis.opendocument.text",
    // Templates — opened as fresh, detached documents.
    "application/vnd.openxmlformats-officedocument.wordprocessingml.template", // .dotx
    "application/vnd.ms-word.template.macroEnabled.12",                        // .dotm
    "application/vnd.oasis.opendocument.text-template",                        // .ott
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

/// Close any open tab whose `path` matches `path`, resetting the active tab to
/// Home when the closed (or a now-shifted) tab was selected.
fn close_tab_for_path(mut tabs: Signal<Vec<OpenTab>>, mut active_tab: Signal<usize>, path: &str) {
    let removed = tabs.read().iter().position(|t| t.path == path);
    if let Some(idx) = removed {
        tabs.write().remove(idx);
        // active_tab is 1-based (index 0 = Home). Reset to Home if the active
        // selection pointed at or past the removed tab to avoid a stale index.
        if *active_tab.read() > idx {
            *active_tab.write() = 0;
        }
    }
}

/// True if `name` has a template extension (Word `.dotx`/`.dotm` or
/// LibreOffice `.ott`/`.ots`). Templates open as fresh, detached documents.
fn is_template_name(name: &str) -> bool {
    name.rsplit('.')
        .next()
        .map(|e| e.to_ascii_lowercase())
        .is_some_and(|e| matches!(e.as_str(), "dotx" | "dotm" | "ott" | "ots"))
}

/// Push `tab` as a new open tab (last position) and return its path so the
/// caller can navigate to the editor.
fn push_new_tab(
    mut tabs: Signal<Vec<OpenTab>>,
    mut active_tab: Signal<usize>,
    tab: OpenTab,
) -> String {
    let path = tab.path.clone();
    tabs.write().push(tab);
    *active_tab.write() = tabs.read().len(); // new tab is last; +1 for Home
    path
}

/// Build a "<stem> Copy.<ext>" filename from a token's display name.
fn suggested_copy_name(token: &FileAccessToken) -> String {
    let name = token.display_name();
    match name.rsplit_once('.') {
        Some((stem, ext)) if !stem.is_empty() => format!("{stem} Copy.{ext}"),
        _ => format!("{name} Copy"),
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
            let path = push_new_tab(tabs, active_tab, new_blank_tab());
            navigator.push(Route::Editor { path });
        }
        // TODO(templates): idx > 0 → push_new_tab(.., new_template_tab(id, name)).
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
                Ok(Some(token)) if is_template_name(token.display_name()) => {
                    // A template (.dotx/.dotm/.ott/.ots): open it as a fresh,
                    // detached document so saving prompts Save As rather than
                    // overwriting the template, and it is not added to recents.
                    let serialized = token.serialize();
                    let title = display_title_from_path(&serialized);
                    let path = push_new_tab(tabs, active_tab, new_import_tab(&serialized, title));
                    nav.push(Route::Editor { path });
                }
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

    // ── on_recent_remove ──────────────────────────────────────────────────────
    let on_recent_remove = move |idx: usize| {
        let path = recent_docs.read().entries.get(idx).map(|e| e.path.clone());
        if let Some(path) = path {
            recent_docs.write().remove(&path);
            recent_docs.read().save();
        }
    };

    // ── on_recent_delete ──────────────────────────────────────────────────────
    //
    // `path` is a serialised FileAccessToken, not a filesystem path. Decode it,
    // delete the underlying file via the capability token, close any open tab
    // for it, then drop the recents entry.
    let on_recent_delete = move |idx: usize| {
        let mut err_sig = pick_error;
        let Some(path) = recent_docs.read().entries.get(idx).map(|e| e.path.clone()) else {
            return;
        };

        match FileAccessToken::deserialize(&path) {
            Ok(token) => {
                if let Err(e) = token.delete() {
                    // Surface the failure but still remove the stale entry — the
                    // file may already be gone or unreachable on this platform.
                    *err_sig.write() = Some(fl!("error-recent-delete-failed", err = e.to_string()));
                }
            }
            Err(_) => {
                *err_sig.write() = Some(fl!("error-recent-invalid-token"));
            }
        }

        close_tab_for_path(tabs, active_tab, &path);
        // TODO(ux): Add a confirmation dialog before destructive deletion.
        recent_docs.write().remove(&path);
        recent_docs.read().save();
    };

    // ── on_recent_open_copy ───────────────────────────────────────────────────
    //
    // The stored `path` is a serialised FileAccessToken. Prompt for a save
    // destination, copy the source bytes into it through the capability tokens
    // (works on every platform), then open the new document.
    let on_recent_open_copy = move |idx: usize| {
        let nav = navigator;
        let mut err_sig = pick_error;
        let tabs = tabs;
        let active_tab = active_tab;
        let mut recent = recent_docs;
        let Some(path) = recent_docs.read().entries.get(idx).map(|e| e.path.clone()) else {
            return;
        };

        spawn(async move {
            let source = match FileAccessToken::deserialize(&path) {
                Ok(t) => t,
                Err(_) => {
                    *err_sig.write() = Some(fl!("error-recent-invalid-token"));
                    return;
                }
            };

            let picker = FilePicker::new();
            let opts = SaveOptions {
                mime_type: MIME_TYPES.first().map(|s| (*s).to_string()),
                suggested_name: Some(suggested_copy_name(&source)),
            };
            let dest = match picker.pick_file_to_save(opts).await {
                Ok(Some(t)) => t,
                Ok(None) => return, // user cancelled
                Err(PickerError::Platform { .. }) => {
                    *err_sig.write() = Some(fl!("error-picker-not-supported"));
                    return;
                }
                Err(e) => {
                    *err_sig.write() = Some(e.to_string());
                    return;
                }
            };

            if let Err(e) = source.copy_bytes_to(&dest) {
                *err_sig.write() = Some(fl!("error-recent-copy-failed", err = e.to_string()));
                return;
            }

            let dest_path = dest.serialize();
            let title = display_title_from_path(&dest_path);
            push_or_switch_tab(tabs, active_tab, dest_path.clone());
            recent.write().record(dest_path.clone(), title);
            recent.read().save();
            nav.push(Route::Editor { path: dest_path });
        });
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
            templates:              make_templates(),
            recent_documents:       recent_list,
            templates_label:        fl!("home-templates-heading"),
            recent_label:           fl!("home-recent-heading"),
            // Empty string hides the Browse card until template browsing is implemented.
            browse_label:           String::new(),
            open_file_label:        fl!("home-open-file"),
            empty_recent_label:     fl!("home-no-recent"),
            recent_menu_aria_label: fl!("home-recent-menu-aria"),
            recent_remove_label:    fl!("home-recent-menu-remove"),
            recent_delete_label:    fl!("home-recent-menu-delete"),
            recent_open_copy_label: fl!("home-recent-menu-open-copy"),
            pick_error:             pick_error,
            on_template_select:     on_template_select,
            // TODO(browse-templates): open a template browser dialog.
            on_browse_templates:    |_| {},
            on_recent_open:         on_recent_open,
            on_open_file:           on_open_file,
            on_recent_remove:       on_recent_remove,
            on_recent_delete:       on_recent_delete,
            on_recent_open_copy:    on_recent_open_copy,
        }
    }
}
