// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 AppThere Loki contributors

//! The editor's status bar, extracted from `editor_inner` (Spec 08 T5.4).
//!
//! Moved out when the zoom badge became the zoom **control**: the bar's props
//! roughly doubled, and `editor_inner` is a baselined over-ceiling file that may
//! not grow. This is the sanctioned cohesive-cluster extraction — the bar is one
//! self-contained region of the tree, and everything it needs arrives as props
//! rather than through `super::`.

use appthere_ui::{AtStatusBar, ZoomCommands};
use dioxus::prelude::*;
use loki_i18n::fl;
use loki_renderer::ViewMode;

use super::editor_state::SaveStatus;

use super::editor_zoom::{FitInputs, actual_size_percent, fit_page_percent, fit_width_percent};

/// Everything the status bar needs from `editor_inner`.
///
/// A struct rather than a dozen positional props, for the reason `ZoomCommands`
/// is one: adjacent same-typed values are where a swap goes unnoticed.
#[derive(Props, Clone, PartialEq)]
pub(super) struct EditorStatusBarProps {
    pub page_label: String,
    pub word_count_label: String,
    pub font_sub_count: i64,
    /// The measurements the fits need, or `None` before the canvas is measured.
    pub fit_inputs: Option<FitInputs>,
    /// The display's CSS pixels per inch, when the platform reported it (T5.5).
    pub css_px_per_inch: Option<f32>,
    /// The capability cap in force, in permille.
    pub zoom_capability_limit_permille: Option<u16>,
    pub zoom_percent: Signal<u32>,
    pub view_mode: Signal<ViewMode>,
    pub view_mode_user_set: Signal<bool>,
    pub font_panel_open: Signal<bool>,
    pub save_message: Signal<Option<SaveStatus>>,
}

/// The status bar.
#[component]
pub(super) fn EditorStatusBar(props: EditorStatusBarProps) -> Element {
    let mut zoom_percent = props.zoom_percent;
    let mut view_mode = props.view_mode;
    let mut view_mode_user_set = props.view_mode_user_set;
    let mut font_panel_open = props.font_panel_open;
    let save_message = props.save_message;
    let font_sub_count = props.font_sub_count;
    let page_label = props.page_label.clone();
    let fit_inputs = props.fit_inputs;
    let css_px_per_inch = props.css_px_per_inch;

    rsx! {
        // ── Status bar ────────────────────────────────────────────────────
        AtStatusBar {
            page_label:         page_label,
            word_count_label:   props.word_count_label.clone(),
            language_label:     fl!("editor-language"),
            zoom_percent:       zoom_percent(),
            zoom_capability_limit_permille: props.zoom_capability_limit_permille,
            // Every row follows a capability that is *checked*, never assumed.
            // Actual Size in particular is wired to the platform probe rather
            // than hardcoded `false`: the row appears the moment the probe
            // reports, and the alternative is a capability that lands and stays
            // invisible (L08-028).
            zoom_commands:      ZoomCommands {
                fit_width:   fit_inputs.is_some(),
                fit_page:    fit_inputs.is_some(),
                actual_size: actual_size_percent(css_px_per_inch).is_some(),
            },
            zoom_labels:        loki_app_shell::zoom_labels::zoom_labels(),
            collaborator_count: 0,
            collaborator_label: String::new(),
            on_zoom_change:     move |p: u32| zoom_percent.set(p),
            on_zoom_fit_width:  move |()| {
                if let Some(p) = fit_inputs.and_then(fit_width_percent) {
                    zoom_percent.set(p);
                }
            },
            on_zoom_fit_page:   move |()| {
                if let Some(p) = fit_inputs.and_then(fit_page_percent) {
                    zoom_percent.set(p);
                }
            },
            on_zoom_actual_size: move |()| {
                if let Some(p) = actual_size_percent(css_px_per_inch) {
                    zoom_percent.set(p);
                }
            },
            view_mode_label:    if view_mode() == ViewMode::Reflow {
                fl!("editor-view-reflowed")
            } else {
                fl!("editor-view-paginated")
            },
            view_mode_aria_label: fl!("editor-view-toggle-aria"),
            on_view_mode_click: move |_| {
                // User override freezes the width-based default.
                view_mode_user_set.set(true);
                let next = if *view_mode.peek() == ViewMode::Reflow {
                    ViewMode::Paginated
                } else {
                    ViewMode::Reflow
                };
                view_mode.set(next);
            },
            // Font-substitution indicator (Spec 03 M3, inverted): the chip
            // is the always-on signal that fonts were substituted; clicking
            // it toggles the detail panel above the ribbon.
            notice_label: if font_sub_count > 0 {
                fl!("editor-font-substitution-chip", count = font_sub_count)
            } else {
                String::new()
            },
            notice_aria_label: fl!("editor-font-substitution-title"),
            on_notice_click:    move |_| {
                let v = *font_panel_open.peek();
                font_panel_open.set(!v);
            },
            // Transient success chip ("Document saved", …). Auto-clears
            // (use_save_status_autoclear) and clears on dirty; click = dismiss.
            status_note_label: super::editor_save_banner::save_status_chip_label(save_message),
            on_status_note_click: {
                let mut save_message = save_message;
                move |_| save_message.set(None)
            },
        }
    }
}
