// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 AppThere Loki contributors

//! Macro **editor** panel (macro spec §3.4, Phase 7) — edit module source and
//! write it back into the document, source-only.
//!
//! Offered only for a document whose macros are enabled (trusted or session).
//! Each module is edited in a multi-line `<textarea>` (Blitz builds a Parley
//! editor for it — see `docs/blitz-textarea-probe.md`). The widget is
//! *uncontrolled*: its initial content seeds from the `value` attribute and edits
//! are read back via `oninput`, so switching module tabs **remounts** the
//! textarea (via `key`) to re-seed. On save the drafts are applied through
//! `loki_vba::write_source` / `loki_odf::basic_write` (never p-code), the edited
//! payload is swapped into the document, trust is re-keyed to the new hash
//! ([`MacroService::reauthor`], §2.5), and a document save is requested.

use std::collections::BTreeMap;
use std::sync::{Arc, Mutex};

use appthere_ui::tokens;
use dioxus::prelude::*;
use loki_i18n::fl;
use loki_macro_host::MacroService;

use super::editor_macro_editor_ops::{build_edited_payload, changed_edits};
use super::editor_macro_notice::{MacroCtx, MacroView, payload_of};
use crate::editing::state::DocumentState;

/// Monospace stack for source editing (shared with the read-only viewer).
const MONO: &str = "ui-monospace, 'Cascadia Code', Consolas, 'Courier New', monospace";

/// Props for [`MacroEditorPanel`].
#[derive(Props, Clone, PartialEq)]
pub(super) struct MacroEditorProps {
    /// Handle to the editor's document state (payload lives on its provenance).
    pub(super) ctx: MacroCtx,
    /// The extracted modules to edit (name + current source).
    pub(super) view: MacroView,
    /// Save-request counter; bumped after a successful edit to persist the
    /// document to disk (the edited payload rides the normal save path).
    pub(super) save_request: Signal<u32>,
    /// Closes the panel.
    pub(super) on_close: EventHandler<()>,
}

/// The macro editor. See the module docs.
///
/// Touch targets: the module tabs, Save, and Close controls meet the 44×44
/// logical-pixel minimum (WCAG 2.5.8) via `min-height`/padding; the textarea is
/// far larger than the minimum.
#[component]
pub(super) fn MacroEditorPanel(props: MacroEditorProps) -> Element {
    let svc = use_context::<MacroService>();
    // Warn before editing a signed project: the edit invalidates the signature
    // (ADR-0014 §4.6).
    let signed = payload_of(&props.ctx.0)
        .map(|p| svc.signature_for(&p).status.is_signed())
        .unwrap_or(false);
    let names: Vec<String> = props.view.modules.iter().map(|m| m.name.clone()).collect();
    let originals: Vec<String> = props
        .view
        .modules
        .iter()
        .map(|m| m.source.clone())
        .collect();

    let mut selected = use_signal(|| 0usize);
    let mut drafts = use_signal(|| originals.clone());
    // None = idle; Some(true) = saved; Some(false) = failed.
    let mut saved = use_signal(|| None::<bool>);

    let idx = selected().min(names.len().saturating_sub(1));
    let current = drafts.read().get(idx).cloned().unwrap_or_default();
    let tab_key = names.get(idx).cloned().unwrap_or_default();

    // Save handler: apply changed drafts, re-key trust, request a document save.
    let ctx_save = props.ctx.clone();
    let svc_save = svc.clone();
    let names_s = names.clone();
    let originals_s = originals.clone();
    let on_close = props.on_close;
    let mut save_request = props.save_request;
    let mut on_save = move || {
        let drafts_now = drafts.read().clone();
        let edits = changed_edits(&names_s, &originals_s, &drafts_now);
        if edits.is_empty() {
            on_close.call(());
            return;
        }
        match apply_edits(&ctx_save.0, &svc_save, &edits) {
            Ok(()) => {
                let next = save_request.peek().wrapping_add(1);
                save_request.set(next);
                saved.set(Some(true));
            }
            Err(e) => {
                tracing::warn!("macro edit save failed: {e}");
                saved.set(Some(false));
            }
        }
    };

    rsx! {
        div { style: container_style(),
            div { style: "display: flex; flex-direction: row; align-items: center; gap: 8px;",
                span {
                    style: format!("font-weight: bold; font-size: {}px;", tokens::FONT_SIZE_MD),
                    {fl!("macros-editor-title")}
                }
                div { style: "flex: 1;" }
                button { style: action_button(true), onclick: move |_| on_save(),
                    {fl!("macros-editor-save")}
                }
                button { style: action_button(false), onclick: move |_| props.on_close.call(()),
                    {fl!("macros-editor-close")}
                }
            }

            span {
                style: format!("font-size: {}px; color: {};", tokens::FONT_SIZE_XS, tokens::COLOR_TEXT_ON_CHROME_SECONDARY),
                {fl!("macros-editor-hint")}
            }

            if signed {
                span {
                    style: format!(
                        "font-size: {}px; color: {}; border-left: 2px solid {c}; padding-left: 6px;",
                        tokens::FONT_SIZE_LABEL, tokens::COLOR_TEXT_ON_CHROME, c = tokens::COLOR_MACRO_BADGE,
                    ),
                    {fl!("macros-sig-edit-warning")}
                }
            }

            if saved() == Some(true) {
                span { style: status_style(false), {fl!("macros-editor-saved")} }
            }
            if saved() == Some(false) {
                span { style: status_style(true), {fl!("macros-editor-failed")} }
            }

            if names.is_empty() {
                span {
                    style: format!("font-size: {}px; color: {};", tokens::FONT_SIZE_LABEL, tokens::COLOR_TEXT_ON_CHROME_SECONDARY),
                    {fl!("macros-viewer-empty")}
                }
            } else {
                div { style: "display: flex; flex-direction: row; flex-wrap: wrap; gap: 6px;",
                    for (i, name) in names.iter().enumerate() {
                        button {
                            key: "{i}",
                            style: tab_style(i == idx),
                            onclick: move |_| { selected.set(i); saved.set(None); },
                            "{name}"
                        }
                    }
                }
                // The textarea is the sole child of this wrapper so its `key` is
                // valid; changing the key on a module switch remounts it, which
                // re-seeds the uncontrolled editor from that module's `value`
                // (probe finding — `docs/blitz-textarea-probe.md`).
                div { style: "display: flex; flex: 1;",
                    textarea {
                        key: "{tab_key}",
                        style: textarea_style(),
                        value: "{current}",
                        oninput: move |e| {
                            let text = e.value();
                            if let Some(slot) = drafts.write().get_mut(idx) {
                                *slot = text;
                            }
                            saved.set(None);
                        },
                    }
                }
            }
        }
    }
}

/// Applies `edits` to the document's payload, re-keys trust, and swaps the new
/// payload into [`DocumentState`] so the next save writes it.
fn apply_edits(
    doc_state: &Arc<Mutex<DocumentState>>,
    svc: &MacroService,
    edits: &BTreeMap<String, String>,
) -> Result<(), String> {
    let original = payload_of(doc_state).ok_or("document has no macro payload")?;
    let edited = build_edited_payload(&original, edits)?;

    let mut guard = doc_state.lock().map_err(|_| "document state is locked")?;
    let Some(doc_arc) = guard.document.as_ref() else {
        return Err("no document loaded".into());
    };
    let mut new_doc = (**doc_arc).clone();
    match new_doc.source.as_mut() {
        Some(src) => src.macros = Some(edited.clone()),
        None => return Err("document has no source provenance".into()),
    }
    guard.document = Some(Arc::new(new_doc));
    guard.generation = guard.generation.wrapping_add(1);
    drop(guard);

    // Re-key trust to the edited hash (self-authored, §2.5). Best-effort and
    // deliberately *not* propagated: the edit is already committed to the
    // document and the caller will request the save, so a trust-store IO failure
    // must not report the whole edit as failed (which would be false) — it just
    // logs. Worst case the re-key is lost and the edited document reopens
    // untrusted, which is the safe (fail-closed) direction.
    if let Err(e) = svc.reauthor(&original, &edited) {
        tracing::warn!("macro edit trust re-key failed (edit still applied): {e}");
    }
    Ok(())
}

fn container_style() -> String {
    format!(
        "display: flex; flex-direction: column; gap: {gap}px; padding: {pv}px {ph}px; \
         background: {bg}; border-top: 1px solid {border}; border-bottom: 1px solid {border}; \
         font-family: {ff}; color: {fg}; flex-shrink: 0; max-height: 55vh;",
        gap = tokens::SPACE_2,
        pv = tokens::SPACE_2,
        ph = tokens::SPACE_4,
        bg = tokens::COLOR_SURFACE_2,
        border = tokens::COLOR_BORDER_CHROME,
        ff = tokens::FONT_FAMILY_UI,
        fg = tokens::COLOR_TEXT_ON_CHROME,
    )
}

fn textarea_style() -> String {
    format!(
        "flex: 1; min-height: 240px; resize: vertical; white-space: pre; \
         font-family: {mono}; font-size: {size}px; color: {fg}; background: {bg}; \
         border: 1px solid {border}; border-radius: {r}px; padding: {p}px;",
        mono = MONO,
        size = tokens::FONT_SIZE_LABEL,
        fg = tokens::COLOR_TEXT_ON_CHROME,
        bg = tokens::COLOR_SURFACE_1,
        border = tokens::COLOR_BORDER_CHROME,
        r = tokens::RADIUS_SM,
        p = tokens::SPACE_2,
    )
}

fn status_style(is_error: bool) -> String {
    let c = if is_error {
        tokens::COLOR_STATUS_ERROR_BORDER
    } else {
        tokens::COLOR_MACRO_BADGE
    };
    format!("font-size: {}px; color: {};", tokens::FONT_SIZE_LABEL, c)
}

fn tab_style(active: bool) -> String {
    let border = if active {
        tokens::COLOR_TAB_ACTIVE_INDICATOR
    } else {
        tokens::COLOR_BORDER_CHROME
    };
    pill(border)
}

fn action_button(accent: bool) -> String {
    let border = if accent {
        tokens::COLOR_MACRO_BADGE
    } else {
        tokens::COLOR_BORDER_CHROME
    };
    pill(border)
}

fn pill(border: &str) -> String {
    format!(
        "min-height: {th}px; padding: {pv}px {ph}px; background: {bg}; \
         border: 1px solid {border}; border-radius: {r}px; color: {fg}; \
         font-size: {size}px; cursor: pointer; flex-shrink: 0;",
        th = tokens::TOUCH_MIN,
        pv = tokens::SPACE_1,
        ph = tokens::SPACE_2,
        bg = tokens::COLOR_SURFACE_3,
        border = border,
        r = tokens::RADIUS_SM,
        fg = tokens::COLOR_TEXT_ON_CHROME,
        size = tokens::FONT_SIZE_LABEL,
    )
}
