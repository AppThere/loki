// SPDX-License-Identifier: Apache-2.0

//! Home screen route component.
//!
//! A thin wrapper over [`appthere_ui::AtHomeTab`] that wires Loki Text's
//! template data, the platform file picker, and the Dioxus router to the
//! generic component's props and callbacks.
//!
//! All user-visible strings come from `loki_i18n::fl!()` — no hardcoded
//! English literals.

use appthere_ui::{AtConfirmDialog, AtHomeTab, BuiltinTemplate, RecentDocument};
use dioxus::prelude::*;
use loki_file_access::{FileAccessToken, FilePicker, PickOptions, PickerError, SaveOptions};
use loki_i18n::fl;

use super::home_util::{
    TemplateBrowserHost, close_tab_for_path, is_template_name, push_new_tab, push_or_switch_tab,
    suggested_copy_name,
};
use crate::new_document::{new_blank_tab, new_import_tab, new_template_tab};
use crate::recent_documents::RecentDocuments;
use crate::routes::Route;
use crate::sessions::DocSessions;
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

// Gallery card 0 is the plain Blank document; cards 1..=5 are the bundled
// templates, in the same order as the `on_template_select` match below.
fn make_templates() -> Vec<BuiltinTemplate> {
    let dotx = || fl!("home-template-format-dotx");
    vec![
        BuiltinTemplate {
            name: fl!("home-template-blank"),
            description: fl!("home-template-blank-description"),
            format_label: fl!("home-template-blank-format"),
        },
        BuiltinTemplate {
            name: fl!("home-template-markdown"),
            description: fl!("home-template-markdown-description"),
            format_label: dotx(),
        },
        BuiltinTemplate {
            name: fl!("home-template-apa"),
            description: fl!("home-template-apa-description"),
            format_label: dotx(),
        },
        BuiltinTemplate {
            name: fl!("home-template-mla"),
            description: fl!("home-template-mla-description"),
            format_label: dotx(),
        },
        BuiltinTemplate {
            name: fl!("home-template-screenplay"),
            description: fl!("home-template-screenplay-description"),
            format_label: dotx(),
        },
        BuiltinTemplate {
            name: fl!("home-template-resume"),
            description: fl!("home-template-resume-description"),
            format_label: dotx(),
        },
    ]
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
    let doc_sessions = use_context::<Signal<DocSessions>>();
    let mut recent_docs = use_context::<Signal<RecentDocuments>>();

    // Holds the last file-picker error message, if any.
    let pick_error: Signal<Option<String>> = use_signal(|| None);

    // True while the template-browser overlay is open (Browse… card).
    let mut browsing_templates = use_signal(|| false);

    // ── on_template_select ── index 0 = Blank, 1..=5 = bundled templates ─────
    let on_template_select = move |idx: usize| {
        let tab = match idx {
            0 => new_blank_tab(),
            1 => new_template_tab("markdown", fl!("home-template-markdown")),
            2 => new_template_tab("apa", fl!("home-template-apa")),
            3 => new_template_tab("mla", fl!("home-template-mla")),
            4 => new_template_tab("screenplay", fl!("home-template-screenplay")),
            5 => new_template_tab("resume", fl!("home-template-resume")),
            _ => return,
        };
        let path = push_new_tab(tabs, active_tab, tab);
        navigator.push(Route::Editor { path });
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
    // Deleting a file is destructive, so the menu action only *requests* it:
    // the confirmation dialog below performs the deletion on confirm (4c.1).
    let mut pending_delete: Signal<Option<(String, String)>> = use_signal(|| None);
    let on_recent_delete = move |idx: usize| {
        let entry = recent_docs
            .read()
            .entries
            .get(idx)
            .map(|e| (e.path.clone(), e.title.clone()));
        if let Some(path_and_title) = entry {
            pending_delete.set(Some(path_and_title));
        }
    };

    // Confirmed deletion: delete the file, close its tab + stashed session, drop recents.
    let mut delete_recent = move |path: String| {
        let mut err_sig = pick_error;
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

        close_tab_for_path(tabs, active_tab, doc_sessions, &path);
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
        // position: relative anchors the AtConfirmDialog overlay over the
        // home area (AtHomeTab sizes itself to the viewport minus tab bar).
        div {
            style: "position: relative;",
            AtHomeTab {
                templates:              make_templates(),
            recent_documents:       recent_list,
            templates_label:        fl!("home-templates-heading"),
            recent_label:           fl!("home-recent-heading"),
            browse_label:           fl!("home-browse-templates"),
            open_file_label:        fl!("home-open-file"),
            empty_recent_label:     fl!("home-no-recent"),
            recent_menu_aria_label: fl!("home-recent-menu-aria"),
            recent_remove_label:    fl!("home-recent-menu-remove"),
            recent_delete_label:    fl!("home-recent-menu-delete"),
            recent_open_copy_label: fl!("home-recent-menu-open-copy"),
                pick_error:             pick_error,
                on_template_select:     on_template_select,
                on_browse_templates:    move |_| browsing_templates.set(true),
                on_recent_open:         on_recent_open,
                on_open_file:           on_open_file,
                on_recent_remove:       on_recent_remove,
                on_recent_delete:       on_recent_delete,
                on_recent_open_copy:    on_recent_open_copy,
            }

            TemplateBrowserHost { browsing: browsing_templates, on_select: on_template_select }

            // ── Delete confirmation (ADR-0013 boundary mount) ─────────────────
            {pending_delete.read().clone().map(|(path, title)| rsx! {
                AtConfirmDialog {
                    title: fl!("home-delete-confirm-title"),
                    message: fl!("home-delete-confirm-message", title = title),
                    confirm_label: fl!("home-delete-confirm-confirm"),
                    cancel_label: fl!("home-delete-confirm-cancel"),
                    on_confirm: move |_| {
                        pending_delete.set(None);
                        delete_recent(path.clone());
                    },
                    on_cancel: move |_| pending_delete.set(None),
                }
            })}
        }
    }
}
