// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 AppThere Loki contributors

//! Document Security panel (macro spec §9.4) — per-document trust state,
//! granted capabilities with revoke, the auto-run-on-open opt-in, and
//! "forget this document".
//!
//! Rendered in-flow above the ribbon (like the read-only viewer); no
//! `position: fixed`. All strings via `fl!()`. The panel reads and records
//! through the ambient [`MacroService`]; it never touches the document bytes.

use appthere_ui::tokens;
use dioxus::prelude::*;
use loki_doc_model::io::macros::MacroPayload;
use loki_i18n::fl;
use loki_macro_host::{Capability, MacroService, TrustDecision};

/// Props for [`MacroSecurityPanel`]. The payload is cloned in (small relative to
/// a user interaction) so handlers can call [`MacroService`] without re-locking
/// the document.
#[derive(Props, Clone, PartialEq)]
pub(super) struct MacroSecurityPanelProps {
    /// The document's preserved macro payload (trust-store key source).
    pub(super) payload: MacroPayload,
    /// The host document title, shown for identity.
    pub(super) title: String,
    /// Whether macros are enabled (so "Run a macro…" / "Edit macros…" are
    /// offered).
    pub(super) can_run: bool,
    /// Opens the macro runner (Tools ▸ Macros).
    pub(super) on_run: EventHandler<()>,
    /// Opens the macro editor (edit + source-only write-back, spec §3.4).
    pub(super) on_edit: EventHandler<()>,
    /// Closes the panel.
    pub(super) on_close: EventHandler<()>,
}

/// The per-document security panel. See the module docs.
///
/// Touch targets: the Close, Revoke, auto-run toggle, and Forget controls meet
/// the 44×44 logical-pixel minimum (WCAG 2.5.8) via `min-height`/padding.
#[component]
pub(super) fn MacroSecurityPanel(props: MacroSecurityPanelProps) -> Element {
    let svc = use_context::<MacroService>();
    // Bumped after each mutation so the panel re-reads the live security state.
    let mut refresh = use_signal(|| 0u32);
    // Subscribe this render to `refresh` so a grant/revoke re-renders the panel.
    let _subscribe = refresh();

    let payload = props.payload.clone();
    let sec = svc.security_for(&payload);

    let state_label = match sec.decision {
        TrustDecision::Disabled => fl!("macros-security-state-disabled"),
        TrustDecision::SessionOnly => fl!("macros-security-state-session"),
        TrustDecision::Trusted => fl!("macros-security-state-trusted"),
    };

    let container = format!(
        "display: flex; flex-direction: column; gap: {gap}px; padding: {pv}px {ph}px; \
         background: {bg}; border-top: 1px solid {border}; border-bottom: 1px solid {border}; \
         font-family: {ff}; color: {fg}; flex-shrink: 0; max-height: 45vh; overflow-y: auto;",
        gap = tokens::SPACE_2,
        pv = tokens::SPACE_2,
        ph = tokens::SPACE_4,
        bg = tokens::COLOR_SURFACE_2,
        border = tokens::COLOR_BORDER_CHROME,
        ff = tokens::FONT_FAMILY_UI,
        fg = tokens::COLOR_TEXT_ON_CHROME,
    );

    let can_auto_run = sec.decision == TrustDecision::Trusted;
    let payload_toggle = payload.clone();
    let svc_toggle = svc.clone();
    let payload_forget = payload.clone();
    let svc_forget = svc.clone();
    let on_close = props.on_close;

    rsx! {
        div { style: "{container}",
            // Header: title + close.
            div {
                style: "display: flex; flex-direction: row; align-items: center; gap: 8px;",
                span {
                    style: format!("font-weight: bold; font-size: {}px;", tokens::FONT_SIZE_MD),
                    {fl!("macros-security-title")}
                }
                div { style: "flex: 1;" }
                if props.can_run {
                    button {
                        style: pill_button(true),
                        onclick: move |_| props.on_run.call(()),
                        {fl!("macros-run-open-action")}
                    }
                    button {
                        style: pill_button(true),
                        onclick: move |_| props.on_edit.call(()),
                        {fl!("macros-edit-open-action")}
                    }
                }
                button {
                    style: pill_button(false),
                    onclick: move |_| on_close.call(()),
                    {fl!("macros-security-close")}
                }
            }

            // Trust state line.
            div {
                style: format!("font-size: {}px; color: {};", tokens::FONT_SIZE_LABEL, tokens::COLOR_TEXT_ON_CHROME_SECONDARY),
                "{fl!(\"macros-security-state-label\")}: {state_label} — {props.title}"
            }

            // Auto-run-on-open opt-in (only meaningful for a trusted document).
            if can_auto_run {
                div {
                    style: "display: flex; flex-direction: column; gap: 4px;",
                    button {
                        style: pill_button(sec.auto_run_open),
                        onclick: move |_| {
                            let next = !sec.auto_run_open;
                            if let Err(e) = svc_toggle.set_auto_run_open(&payload_toggle, next) {
                                tracing::warn!("macro trust save failed: {e}");
                            }
                            refresh.set(refresh() + 1);
                        },
                        {if sec.auto_run_open { "☑ " } else { "☐ " }}
                        {fl!("macros-security-auto-run")}
                    }
                    span {
                        style: format!("font-size: {}px; color: {};", tokens::FONT_SIZE_XS, tokens::COLOR_TEXT_ON_CHROME_SECONDARY),
                        {fl!("macros-security-auto-run-hint")}
                    }
                }
            }

            // Capability grants.
            span {
                style: format!("font-weight: bold; font-size: {}px; margin-top: 4px;", tokens::FONT_SIZE_LABEL),
                {fl!("macros-security-capabilities")}
            }
            if sec.capabilities.iter().all(|c| !c.granted() && !c.refused) {
                span {
                    style: format!("font-size: {}px; color: {};", tokens::FONT_SIZE_LABEL, tokens::COLOR_TEXT_ON_CHROME_SECONDARY),
                    {fl!("macros-security-cap-none")}
                }
            }
            for cap_state in sec.capabilities.iter().filter(|c| c.granted() || c.refused).cloned() {
                {
                    let svc_row = svc.clone();
                    let payload_row = payload.clone();
                    let cap = cap_state.capability;
                    rsx! {
                        div {
                            key: "{cap.id()}",
                            style: "display: flex; flex-direction: row; align-items: center; gap: 8px;",
                            span {
                                style: format!("flex: 1; font-size: {}px;", tokens::FONT_SIZE_LABEL),
                                {capability_name(cap)}
                                {cap_tag(&cap_state)}
                            }
                            if cap_state.granted() {
                                button {
                                    style: pill_button(false),
                                    onclick: move |_| {
                                        if let Err(e) = svc_row.revoke(&payload_row, cap) {
                                            tracing::warn!("macro trust save failed: {e}");
                                        }
                                        refresh.set(refresh() + 1);
                                    },
                                    {fl!("macros-security-cap-revoke")}
                                }
                            }
                        }
                    }
                }
            }

            // Forget this document.
            div { style: "flex: 1;" }
            if sec.has_record {
                button {
                    style: format!("{base} border-color: {c}; color: {c};", base = pill_button(false), c = tokens::COLOR_STATUS_ERROR_BORDER),
                    onclick: move |_| {
                        if let Err(e) = svc_forget.forget(&payload_forget) {
                            tracing::warn!("macro trust save failed: {e}");
                        }
                        refresh.set(refresh() + 1);
                    },
                    {fl!("macros-security-forget")}
                }
            }
        }
    }
}

/// The short, human capability name for `cap` (`macros-cap-<id>-name`).
fn capability_name(cap: Capability) -> String {
    loki_i18n::bundle().get(&format!("macros-cap-{}-name", cap.id()), None)
}

/// A parenthetical tag noting the grant scope or refusal for a capability row.
fn cap_tag(state: &loki_macro_host::CapabilityState) -> String {
    if state.refused {
        format!(" ({})", fl!("macros-security-cap-refused"))
    } else if state.persisted {
        format!(" ({})", fl!("macros-security-cap-always-tag"))
    } else if state.session {
        format!(" ({})", fl!("macros-security-cap-session-tag"))
    } else {
        String::new()
    }
}

/// A pill button style; `active` draws the macro-badge accent border.
fn pill_button(active: bool) -> String {
    let border = if active {
        tokens::COLOR_MACRO_BADGE
    } else {
        tokens::COLOR_BORDER_CHROME
    };
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
