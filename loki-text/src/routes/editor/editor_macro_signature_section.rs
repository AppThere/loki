// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 AppThere Loki contributors

//! Signature section of the Document Security panel (8A.7; ADR-0014 §5).
//!
//! Shows the document's macro-signature state (🔏 signed by whom, trusted or
//! not), offers "Trust this publisher…" behind an anti-spoof-framed confirm, and
//! renders the pinned-publisher management list with per-row Remove. All strings
//! via `fl!()`; all state flows through the ambient [`MacroService`] — never the
//! document bytes (T10).

use appthere_ui::tokens;
use dioxus::prelude::*;
use loki_doc_model::io::macros::MacroPayload;
use loki_i18n::fl;
use loki_macro_host::{MacroService, PublisherRecord, SignatureStatus, SignatureSummary};

/// Props for [`MacroSignatureSection`].
#[derive(Props, Clone, PartialEq)]
pub(super) struct MacroSignatureSectionProps {
    /// The document's preserved macro payload (signature-store key source).
    pub(super) payload: MacroPayload,
}

/// The signature + trusted-publisher section. See the module docs.
///
/// Touch targets: the Trust, Confirm/Cancel, and Remove controls meet the
/// 44×44 logical-pixel minimum (WCAG 2.5.8) via `min-height`/padding.
#[component]
pub(super) fn MacroSignatureSection(props: MacroSignatureSectionProps) -> Element {
    let svc = use_context::<MacroService>();
    let mut refresh = use_signal(|| 0u32);
    let _subscribe = refresh();
    // Inline "are you sure?" step for pinning (the anti-spoof gate, §5.5).
    let mut confirming = use_signal(|| false);

    let payload = props.payload.clone();
    let summary = svc.signature_for(&payload);
    let publishers = svc.trusted_publishers();

    // Unsigned documents get no signature UI at all.
    if summary.status == SignatureStatus::Unsigned && publishers.is_empty() {
        return rsx! {};
    }

    let name = summary.signer_cn.clone().unwrap_or_default();
    let svc_pin = svc.clone();
    let payload_pin = payload.clone();

    rsx! {
        div {
            style: section_style(),
            // ── Signature state line ─────────────────────────────────────────
            if summary.status.is_signed() {
                div {
                    style: "display: flex; flex-direction: column; gap: 2px;",
                    span {
                        style: format!("font-size: {}px; color: {};", tokens::FONT_SIZE_LABEL, status_color(summary.status)),
                        {status_line(&summary, &name)}
                    }
                    {detail_lines(&summary)}
                    if summary.status.is_trusted() {
                        span {
                            style: hint_style(),
                            {fl!("macros-sig-trusted-note")}
                        }
                    }
                }

                // ── Trust-this-publisher affordance (behind a confirm) ────────
                if summary.status.can_pin() {
                    if confirming() {
                        div {
                            style: confirm_box_style(),
                            span {
                                style: format!("font-size: {}px;", tokens::FONT_SIZE_LABEL),
                                {fl!("macros-sig-trust-message", name = name.clone())}
                            }
                            div {
                                style: "display: flex; flex-direction: row; gap: 8px;",
                                button {
                                    style: pill(true),
                                    onclick: move |_| {
                                        if let Err(e) = svc_pin.pin_publisher(&payload_pin) {
                                            tracing::warn!("trusted-publisher save failed: {e}");
                                        }
                                        confirming.set(false);
                                        refresh.set(refresh() + 1);
                                    },
                                    {fl!("macros-sig-trust-confirm")}
                                }
                                button {
                                    style: pill(false),
                                    onclick: move |_| confirming.set(false),
                                    {fl!("macros-sig-trust-cancel")}
                                }
                            }
                        }
                    } else {
                        button {
                            style: pill(true),
                            onclick: move |_| confirming.set(true),
                            {trust_action_label(summary.status)}
                        }
                    }
                }
            }

            // ── Trusted-publisher management list ─────────────────────────────
            if !publishers.is_empty() {
                span {
                    style: format!("font-weight: bold; font-size: {}px; margin-top: 4px;", tokens::FONT_SIZE_LABEL),
                    {fl!("macros-sig-manage-title")}
                }
                for record in publishers {
                    {publisher_row(&svc, record, refresh)}
                }
                span {
                    style: hint_style(),
                    {fl!("macros-sig-manage-revocation-note")}
                }
            }
        }
    }
}

/// One management-list row: display name, issuer, short thumbprint, and Remove.
fn publisher_row(svc: &MacroService, record: PublisherRecord, mut refresh: Signal<u32>) -> Element {
    let svc = svc.clone();
    let thumbprint = record.thumbprint;
    rsx! {
        div {
            key: "{short_hex(&thumbprint)}",
            style: "display: flex; flex-direction: row; align-items: center; gap: 8px;",
            div {
                style: "display: flex; flex-direction: column; flex: 1; gap: 1px;",
                span {
                    style: format!("font-size: {}px;", tokens::FONT_SIZE_LABEL),
                    "{record.display_name}"
                }
                span {
                    style: hint_style(),
                    {fl!("macros-sig-issuer", issuer = record.issuer.clone())}
                }
            }
            button {
                style: format!("{base} border-color: {c}; color: {c};", base = pill(false), c = tokens::COLOR_STATUS_ERROR_BORDER),
                onclick: move |_| {
                    if let Err(e) = svc.unpin_publisher(&thumbprint) {
                        tracing::warn!("trusted-publisher save failed: {e}");
                    }
                    refresh.set(refresh() + 1);
                },
                {fl!("macros-sig-manage-remove")}
            }
        }
    }
}

/// The status headline for a signed document.
fn status_line(summary: &SignatureSummary, name: &str) -> String {
    let icon = "🔏 ";
    let body = match summary.status {
        SignatureStatus::Trusted => fl!("macros-sig-trusted", name = name.to_owned()),
        SignatureStatus::Untrusted => fl!("macros-sig-untrusted", name = name.to_owned()),
        SignatureStatus::Legacy => fl!("macros-sig-legacy", name = name.to_owned()),
        SignatureStatus::Expired => fl!("macros-sig-expired", name = name.to_owned()),
        SignatureStatus::Renewed => fl!("macros-sig-renewed", name = name.to_owned()),
        SignatureStatus::Invalid => return format!("⚠ {}", fl!("macros-sig-invalid")),
        SignatureStatus::Unsigned => return fl!("macros-sig-unsigned"),
    };
    format!("{icon}{body}")
}

/// Issuer + thumbprint detail rows (only when a signer is present).
fn detail_lines(summary: &SignatureSummary) -> Element {
    let issuer = summary.issuer.clone();
    let thumbprint = summary.thumbprint_hex.clone();
    rsx! {
        if let Some(issuer) = issuer {
            span { style: hint_style(), {fl!("macros-sig-issuer", issuer = issuer)} }
        }
        if let Some(tp) = thumbprint {
            span { style: hint_style(), {fl!("macros-sig-thumbprint", thumbprint = tp)} }
        }
    }
}

/// The pin-button label (initial trust vs. re-pin after a renewal).
fn trust_action_label(status: SignatureStatus) -> String {
    if status == SignatureStatus::Renewed {
        fl!("macros-sig-trust-renewed-action")
    } else {
        fl!("macros-sig-trust-action")
    }
}

/// First eight bytes of a thumbprint as hex, for the row key.
fn short_hex(bytes: &[u8; 32]) -> String {
    bytes.iter().take(8).map(|b| format!("{b:02x}")).collect()
}

/// Accent colour for the status headline by trust state.
fn status_color(status: SignatureStatus) -> &'static str {
    match status {
        SignatureStatus::Trusted => tokens::COLOR_MACRO_BADGE,
        SignatureStatus::Invalid => tokens::COLOR_STATUS_ERROR_BORDER,
        _ => tokens::COLOR_TEXT_ON_CHROME,
    }
}

fn section_style() -> String {
    format!(
        "display: flex; flex-direction: column; gap: {gap}px; padding-top: {gap}px; \
         border-top: 1px solid {border};",
        gap = tokens::SPACE_2,
        border = tokens::COLOR_BORDER_CHROME,
    )
}

fn hint_style() -> String {
    format!(
        "font-size: {}px; color: {};",
        tokens::FONT_SIZE_XS,
        tokens::COLOR_TEXT_ON_CHROME_SECONDARY
    )
}

fn confirm_box_style() -> String {
    format!(
        "display: flex; flex-direction: column; gap: {gap}px; padding: {pv}px {ph}px; \
         border: 1px solid {badge}; border-radius: {r}px;",
        gap = tokens::SPACE_1,
        pv = tokens::SPACE_2,
        ph = tokens::SPACE_2,
        badge = tokens::COLOR_MACRO_BADGE,
        r = tokens::RADIUS_SM,
    )
}

/// A pill button; `active` draws the macro-badge accent border.
fn pill(active: bool) -> String {
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
        r = tokens::RADIUS_SM,
        fg = tokens::COLOR_TEXT_ON_CHROME,
        size = tokens::FONT_SIZE_LABEL,
    )
}
