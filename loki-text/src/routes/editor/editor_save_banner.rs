// SPDX-License-Identifier: Apache-2.0

//! Transient status surfaces for save/export/insert results.
//!
//! Successes render as an auto-clearing chip in the status bar (wired in
//! `editor_inner`; the auto-clear lives in [`use_save_status_autoclear`]).
//! Errors render here as a persistent banner below the panels, above the
//! ribbon, dismissed manually — failures must not silently vanish.

use std::time::Duration;

use appthere_ui::tokens;
use dioxus::prelude::*;

use super::editor_state::SaveStatus;

/// How long a success chip stays before clearing itself.
const AUTO_CLEAR_MS: u64 = 4000;

/// Renders the error banner when `save_message` holds an error, with a close
/// button that clears it. Successes are the status-bar chip, not this banner.
pub(super) fn save_banner(mut save_message: Signal<Option<SaveStatus>>) -> Element {
    let msg = match save_message.read().as_ref() {
        Some(status) if status.is_error => status.text.clone(),
        _ => return rsx! {},
    };
    rsx! {
        div {
            style: format!(
                "display: flex; flex-direction: row; align-items: center; \
                 justify-content: space-between; padding: {p}px {p2}px; \
                 background: {bg}; border-top: 1px solid {border}; \
                 font-family: {ff}; font-size: {size}px; \
                 color: {fg}; flex-shrink: 0;",
                p = tokens::SPACE_2,
                p2 = tokens::SPACE_4,
                bg = tokens::COLOR_SURFACE_2,
                border = tokens::COLOR_BORDER_CHROME,
                ff = tokens::FONT_FAMILY_UI,
                size = tokens::FONT_SIZE_LABEL,
                fg = tokens::COLOR_TEXT_ON_CHROME,
            ),
            span { "{msg}" }
            button {
                style: format!(
                    "background: transparent; border: none; font-size: {fs}px; \
                     color: {fg}; cursor: pointer; padding: {p}px;",
                    fs = tokens::FONT_SIZE_LABEL,
                    fg = tokens::COLOR_TEXT_ON_CHROME_SECONDARY,
                    p = tokens::SPACE_1,
                ),
                onclick: move |_| { save_message.set(None); },
                "\u{2715}"
            }
        }
    }
}

/// The status-bar chip label: the current *success* status text, or empty
/// (which hides the chip). Errors render in the banner instead.
pub(super) fn save_status_chip_label(save_message: Signal<Option<SaveStatus>>) -> String {
    save_message
        .read()
        .as_ref()
        .filter(|status| !status.is_error)
        .map(|status| status.text.clone())
        .unwrap_or_default()
}

/// Clears a *success* status a few seconds after it appears (errors persist
/// until dismissed). A worker thread sleeps and signals back through a oneshot
/// — the same cross-thread yield pattern as the open-path layout task — and
/// the clear is skipped if a newer status replaced this one meanwhile.
pub(super) fn use_save_status_autoclear(mut save_message: Signal<Option<SaveStatus>>) {
    use_effect(move || {
        let Some(status) = save_message.read().clone() else {
            return;
        };
        if status.is_error {
            return;
        }
        let (tx, rx) = futures_channel::oneshot::channel();
        let spawned = std::thread::Builder::new()
            .name("loki-status-clear".into())
            .spawn(move || {
                std::thread::sleep(Duration::from_millis(AUTO_CLEAR_MS));
                let _ = tx.send(());
            });
        if spawned.is_ok() {
            spawn(async move {
                if rx.await.is_ok() && save_message.peek().as_ref() == Some(&status) {
                    save_message.set(None);
                }
            });
        }
    });
}
