// SPDX-License-Identifier: Apache-2.0

//! Publish tab ribbon content (Spec 04 M3/M4).
//!
//! [`publish_tab_content`] builds the Publish tab's groups as
//! [`RibbonGroupSpec`]s wrapped in [`AtRibbonGroups`], so the width-driven
//! collapse cascade + overflow menu drive them like every other tab. The export
//! actions themselves ([`run_export`]) and the PDF/X level panel live in
//! [`super::editor_publish`].

use std::sync::{Arc, Mutex};

use super::editor_state::SaveStatus;
use appthere_ui::{
    AtRibbonGroups, AtRibbonIconButton, RibbonGroupSpec, estimate_group_metrics, tokens,
};
use dioxus::prelude::*;
use loki_i18n::fl;

use super::editor_metadata::{MetaDraft, meta_to_draft};
use super::editor_publish::{PublishFormat, run_export};
use crate::editing::state::DocumentState;

/// Builds the Publish tab ribbon content (Export + Metadata groups).
pub(super) fn publish_tab_content(
    doc_state: &Arc<Mutex<DocumentState>>,
    path_signal: Signal<String>,
    save_message: Signal<Option<SaveStatus>>,
    mut is_publish_panel_open: Signal<bool>,
    mut editing_metadata: Signal<Option<MetaDraft>>,
) -> Element {
    let ds_epub = Arc::clone(doc_state);
    let ds_meta = Arc::clone(doc_state);

    // Export (PDF/X + EPUB) is kept full longer than the single Metadata button.
    let export = RibbonGroupSpec {
        metrics: estimate_group_metrics(1, 2, true),
        label: Some(fl!("publish-group-export")),
        aria_label: fl!("publish-group-export"),
        content: rsx! {
            AtRibbonIconButton {
                aria_label: fl!("publish-export-pdf-aria"),
                is_active: is_publish_panel_open(),
                is_disabled: false,
                on_click: move |_| {
                    let open = is_publish_panel_open();
                    is_publish_panel_open.set(!open);
                },
                {label_node(&fl!("publish-export-pdf-label"))}
            }
            AtRibbonIconButton {
                aria_label: fl!("publish-export-epub-aria"),
                is_active: false,
                is_disabled: false,
                on_click: move |_| {
                    run_export(&ds_epub, PublishFormat::Epub, &path_signal.peek(), save_message);
                },
                {label_node(&fl!("publish-export-epub-label"))}
            }
        },
    };

    let metadata = RibbonGroupSpec {
        metrics: estimate_group_metrics(0, 1, true),
        label: Some(fl!("publish-group-metadata")),
        aria_label: fl!("publish-group-metadata"),
        content: rsx! {
            AtRibbonIconButton {
                aria_label: fl!("publish-metadata-aria"),
                is_active: editing_metadata.read().is_some(),
                is_disabled: false,
                on_click: move |_| {
                    if editing_metadata.read().is_some() {
                        editing_metadata.set(None);
                    } else {
                        editing_metadata.set(Some(meta_to_draft(&ds_meta)));
                    }
                },
                {label_node(&fl!("publish-metadata-label"))}
            }
        },
    };

    rsx! {
        AtRibbonGroups {
            overflow_aria_label: fl!("ribbon-overflow-aria"),
            groups: vec![export, metadata],
        }
    }
}

/// Renders a compact text label inside a ribbon button (these actions have no
/// dedicated icon).
fn label_node(text: &str) -> Element {
    rsx! {
        span {
            style: format!(
                "font-family: {ff}; font-size: {fs}px; color: inherit; \
                 white-space: nowrap;",
                ff = tokens::FONT_FAMILY_UI,
                fs = tokens::FONT_SIZE_LABEL,
            ),
            "{text}"
        }
    }
}
