// SPDX-License-Identifier: Apache-2.0

//! Publish tab ribbon content (Spec 04 M3/M4).
//!
//! [`publish_tab_content`] builds the Publish tab's groups as
//! [`RibbonGroupSpec`]s wrapped in [`AtRibbonGroups`], so the width-driven
//! collapse cascade + overflow menu drive them like every other tab. The export
//! actions themselves ([`super::editor_publish::run_export`]) and the PDF/X
//! level panel live in
//! [`super::editor_publish`].

use appthere_ui::{
    AtRibbonGroups, AtRibbonIconButton, RibbonGroupSpec, estimate_group_metrics, tokens,
};
use dioxus::prelude::*;
use loki_i18n::fl;

/// Builds the Publish tab ribbon content (Export + Metadata groups).
///
/// `dialogs` carries the two tabbed dialogs this tab opens: the EPUB button now
/// opens the **Publish EPUB 3** dialog (design section 7) rather than exporting
/// straight away — the export itself still runs through
/// [`super::editor_publish::run_export`], from
/// the dialog's Publish button, once the standing preflight has been seen.
/// Takes no document handle: all three buttons now open a surface that reads the
/// document itself — the PDF/X level panel, the EPUB dialog, the properties
/// dialog — where this tab used to build a metadata draft inline.
pub(super) fn publish_tab_content(
    mut is_publish_panel_open: Signal<bool>,
    dialogs: super::editor_dialog_state::DialogSignals,
) -> Element {
    let mut publish_epub = dialogs.publish_epub;
    let mut metadata = dialogs.metadata;
    let mut print_open = dialogs.print;

    // Export (PDF/X + EPUB) is kept full longer than the single Metadata button.
    let export = RibbonGroupSpec {
        metrics: estimate_group_metrics(1, 3, true),
        partial: None,
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
                aria_label: fl!("publish-print-aria"),
                is_active: print_open(),
                is_disabled: false,
                on_click: move |_| {
                    let is_open = print_open();
                    print_open.set(!is_open);
                },
                {label_node(&fl!("publish-print-label"))}
            }
            AtRibbonIconButton {
                aria_label: fl!("publish-export-epub-aria"),
                is_active: publish_epub(),
                is_disabled: false,
                on_click: move |_| {
                    let is_open = publish_epub();
                    publish_epub.set(!is_open);
                },
                {label_node(&fl!("publish-export-epub-label"))}
            }
        },
    };

    let metadata_group = RibbonGroupSpec {
        metrics: estimate_group_metrics(0, 1, true),
        partial: None,
        label: Some(fl!("publish-group-metadata")),
        aria_label: fl!("publish-group-metadata"),
        content: rsx! {
            // The tabbed document properties dialog (design section 4): the
            // Dublin Core fields the docked panel carried, plus identifiers, an
            // accessibility review and live statistics. It replaced that panel
            // rather than sitting beside it — one door to one document's
            // properties.
            AtRibbonIconButton {
                aria_label: fl!("publish-metadata-aria"),
                is_active: metadata(),
                is_disabled: false,
                on_click: move |_| {
                    let is_open = metadata();
                    metadata.set(!is_open);
                },
                {label_node(&fl!("publish-metadata-label"))}
            }
        },
    };

    rsx! {
        AtRibbonGroups {
            overflow_aria_label: fl!("ribbon-overflow-aria"),
            groups: vec![export, metadata_group],
        }
    }
}

/// Renders a compact text label inside a ribbon button (these actions have no
/// dedicated icon).
fn label_node(text: &str) -> Element {
    rsx! {
        span {
            style: format!(
                "font-size: {fs}px; color: inherit; \
                 white-space: nowrap;",
                fs = tokens::FONT_SIZE_LABEL,
            ),
            "{text}"
        }
    }
}
