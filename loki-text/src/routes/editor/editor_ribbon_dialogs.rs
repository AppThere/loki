// SPDX-License-Identifier: Apache-2.0

//! The Format tab's entry point into the span formatting dialog, and the text
//! label the dialog-opening ribbon buttons share.
//!
//! # Why these are labelled rather than iconned
//!
//! The icon set has no glyph for "character formatting" that the Font colour
//! and Highlight triggers beside it would not also answer to, and an icon that
//! lied about which of the three you were pressing would be worse than a word
//! that does not. The Insert and Publish tabs' dialog buttons kept the icons and
//! labels of the one-click actions they replaced, because they *are* those
//! actions now — there is nothing left to tell them apart from.

use appthere_ui::{AtRibbonIconButton, RibbonGroupSpec, estimate_group_metrics, tokens};
use dioxus::prelude::*;
use loki_i18n::fl;

/// A compact text label inside a ribbon button, for actions with no icon.
pub(super) fn dialog_label(text: &str) -> Element {
    rsx! {
        span {
            style: format!(
                "font-size: {fs}px; color: inherit; white-space: nowrap;",
                fs = tokens::FONT_SIZE_LABEL,
            ),
            "{text}"
        }
    }
}

/// The Format tab's **Character** group — opens the span formatting dialog.
pub(super) fn character_group(mut open: Signal<bool>, priority: u8) -> RibbonGroupSpec {
    RibbonGroupSpec {
        metrics: estimate_group_metrics(priority, 1, true),
        partial: None,
        label: Some(fl!("ribbon-group-character")),
        aria_label: fl!("ribbon-group-character"),
        content: rsx! {
            AtRibbonIconButton {
                aria_label: fl!("ribbon-character-dialog-aria"),
                is_active: open(),
                is_disabled: false,
                on_click: move |_| {
                    let is_open = open();
                    open.set(!is_open);
                },
                {dialog_label(&fl!("ribbon-character-dialog-label"))}
            }
        },
    }
}
