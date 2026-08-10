// SPDX-License-Identifier: Apache-2.0

//! Label and table-cell helpers for the Tab stops tab, split from
//! [`super::tab_stops`] to keep both files under the 300-line ceiling.

use appthere_ui::{at_field_label_style, tokens};
use dioxus::prelude::*;
use loki_doc_model::style::props::tab_stop::{TabAlignment, TabLeader};
use loki_i18n::fl;

/// The localized label for a tab alignment.
pub(super) fn alignment_label(a: TabAlignment) -> String {
    match a {
        TabAlignment::Right => fl!("style-dialog-stops-align-right"),
        TabAlignment::Center => fl!("style-dialog-stops-align-centre"),
        TabAlignment::Decimal => fl!("style-dialog-stops-align-decimal"),
        TabAlignment::Clear => fl!("style-dialog-stops-align-clear"),
        _ => fl!("style-dialog-stops-align-left"),
    }
}

/// The localized label for a tab leader.
pub(super) fn leader_label(l: TabLeader) -> String {
    match l {
        TabLeader::Dot | TabLeader::MiddleDot => fl!("style-dialog-stops-leader-dotted"),
        TabLeader::Dash => fl!("style-dialog-stops-leader-dashed"),
        TabLeader::Underscore | TabLeader::Heavy => fl!("style-dialog-stops-leader-underscore"),
        _ => fl!("style-dialog-stops-leader-none"),
    }
}

/// A table header cell.
pub(super) fn header_cell(text: String, extra: &str) -> Element {
    rsx! {
        div {
            style: format!(
                "{extra} padding: {py}px {px}px; {label}",
                py = tokens::SPACE_2,
                px = tokens::SPACE_3,
                label = at_field_label_style(),
            ),
            {text}
        }
    }
}

/// A table body cell.
pub(super) fn body_cell(text: String, extra: &str) -> Element {
    rsx! {
        div {
            style: format!(
                "{extra} padding: {py}px {px}px; min-width: 0;",
                py = tokens::SPACE_2,
                px = tokens::SPACE_3,
            ),
            {text}
        }
    }
}
