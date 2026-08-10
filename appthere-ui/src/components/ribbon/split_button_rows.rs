// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 AppThere Loki contributors

//! The split-button menu's row renderer (split from `split_button.rs` for the
//! 300-line ceiling).

use std::rc::Rc;

use dioxus::prelude::*;

use super::{SplitMenuItem, ACTIVE_ROW_BG};
use crate::tokens::colors::{COLOR_SURFACE_PAGE, COLOR_TEXT_PRIMARY};
use crate::tokens::spacing::{RADIUS_SM, SPACE_2, SPACE_3, TOUCH_MIN};
use crate::tokens::typography::FONT_SIZE_BODY;

/// The menu's rows — one table for the pointer and the keyboard (see the
/// Recent menu's module docs for why that is load-bearing).
pub(super) fn menu_rows(
    items: &[SplitMenuItem],
    on_item: EventHandler<String>,
    dismiss: &Rc<dyn Fn()>,
    active: Option<usize>,
) -> Element {
    rsx! {
        div {
            style: format!(
                "display: flex; flex-direction: column; padding: {SPACE_2}px; \
                 background: {COLOR_SURFACE_PAGE}; border-radius: {RADIUS_SM}px;"
            ),
            for (row, item) in items.iter().enumerate() {
                button {
                    key: "{item.id}",
                    style: format!(
                        "display: flex; align-items: center; min-height: {TOUCH_MIN}px; \
                         padding: 0 {SPACE_3}px; border: none; text-align: left; \
                         border-radius: {RADIUS_SM}px; cursor: pointer; \
                         font-size: {FONT_SIZE_BODY}px; color: {COLOR_TEXT_PRIMARY}; \
                         background: {bg};",
                        bg = if active == Some(row) { ACTIVE_ROW_BG } else { "transparent" },
                    ),
                    onclick: {
                        let id = item.id.clone();
                        let dismiss = Rc::clone(dismiss);
                        move |_| {
                            dismiss();
                            on_item.call(id.clone());
                        }
                    },
                    "{item.label}"
                }
            }
        }
    }
}
