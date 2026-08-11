// SPDX-License-Identifier: Apache-2.0

//! Format-tab colour-picker trigger groups (**Font colour** / **Highlight**)
//! and the shared colour palettes.
//!
//! Each group is a single [`AtColorPickerTrigger`] whose click toggles the
//! docked picker panel (`editor_color_panel`) above the ribbon — the ribbon
//! content row is a scroll container, so the panel cannot be anchored inside
//! it (see the `appthere_ui` colour-picker module docs).
//!
//! Highlight accepts **any** colour (Spec 08 T5.3). A colour that exactly
//! matches one of the sixteen `w:highlight` names is stored as that name;
//! anything else is stored as character shading. See `editor_highlight_color`
//! for the routing and what it means in Word.

use appthere_ui::{
    AtColorPickerTrigger, AtColorSwatch, AtIcon, LUCIDE_BASELINE, LUCIDE_HIGHLIGHTER,
    RibbonGroupSpec, estimate_group_metrics,
};
use dioxus::prelude::*;
use loki_doc_model::style::props::HIGHLIGHT_RGB;
use loki_i18n::fl;

use super::editor_state::ColorPickerTarget;

/// Recent colours kept per picker (most recent first).
const RECENT_MAX: usize = 6;

/// Preset text colours as `(value, fill, aria-key)` — fill == value (a hex).
/// Readable on a white page.
pub(super) const FONT_COLOR_PALETTE: &[(&str, &str, &str)] = &[
    ("#C0392B", "#C0392B", "ribbon-color-red-aria"),
    ("#E67E22", "#E67E22", "ribbon-color-orange-aria"),
    ("#F1C40F", "#F1C40F", "ribbon-color-yellow-aria"),
    ("#27AE60", "#27AE60", "ribbon-color-green-aria"),
    ("#2980B9", "#2980B9", "ribbon-color-blue-aria"),
    ("#8E44AD", "#8E44AD", "ribbon-color-purple-aria"),
];

/// The aria key for each named highlight, **in `HIGHLIGHT_RGB`'s order**.
///
/// Only the labels are here. The colours are `loki-doc-model`'s
/// [`HIGHLIGHT_RGB`], which is also what the layout paints and what the
/// hex→variant lookup reads — one fact, one derivation (L08-029). This file
/// used to carry its own sixteen hex strings with a comment saying they
/// "mirror" the layout's sixteen float triples.
///
/// Paired by *position*, which `highlight_swatches` asserts rather than
/// assumes: a table of `(variant, key)` pairs would be a second statement of
/// which variants exist, and the one that drifts is always the second.
const HIGHLIGHT_ARIA: &[&str] = &[
    "ribbon-highlight-yellow-aria",
    "ribbon-highlight-green-aria",
    "ribbon-highlight-cyan-aria",
    "ribbon-highlight-magenta-aria",
    "ribbon-highlight-blue-aria",
    "ribbon-highlight-red-aria",
    "ribbon-highlight-dark-blue-aria",
    "ribbon-highlight-dark-cyan-aria",
    "ribbon-highlight-dark-green-aria",
    "ribbon-highlight-dark-magenta-aria",
    "ribbon-highlight-dark-red-aria",
    "ribbon-highlight-dark-yellow-aria",
    "ribbon-highlight-dark-gray-aria",
    "ribbon-highlight-light-gray-aria",
    "ribbon-highlight-black-aria",
    "ribbon-highlight-white-aria",
];

/// The named highlights as picker swatches.
///
/// **The swatch value is the hex, not the variant name** (Spec 08 T5.3). It was
/// the variant name while highlight was restricted to the palette; now that a
/// custom colour is offered, a swatch carrying a name and a hex field carrying a
/// hex would be two kinds of pick for the picker to tell apart — and telling
/// them apart is exactly what makes the same yellow export two different ways.
/// So there is one kind of pick, and `apply_highlight` resolves it.
///
/// Pairs by position with [`HIGHLIGHT_ARIA`]; a mismatch drops the tail rather
/// than mislabelling a swatch, and `highlight_swatches_are_complete` fails on it.
pub(super) fn highlight_swatches() -> Vec<AtColorSwatch> {
    HIGHLIGHT_RGB
        .iter()
        .zip(HIGHLIGHT_ARIA)
        .map(|((_, hex), aria)| AtColorSwatch {
            value: (*hex).to_string(),
            fill: (*hex).to_string(),
            aria_label: fl!(aria),
        })
        .collect()
}

/// A preset palette as picker swatches (aria labels resolved via `fl!`).
pub(super) fn preset_swatches(palette: &'static [(&str, &str, &str)]) -> Vec<AtColorSwatch> {
    palette
        .iter()
        .map(|(value, fill, aria)| AtColorSwatch {
            value: (*value).to_string(),
            fill: (*fill).to_string(),
            aria_label: fl!(aria),
        })
        .collect()
}

/// Stored colours → swatches.
///
/// The fill *is* the value: every colour on every route — text, highlight,
/// recent, document — is a `#RRGGBB` since T5.3. The parameterised
/// value-to-fill lookup this replaced is what made the Highlight picker's
/// document group paint transparent swatches.
pub(super) fn recent_swatches(values: &[String]) -> Vec<AtColorSwatch> {
    values
        .iter()
        .map(|v| AtColorSwatch {
            value: v.clone(),
            fill: v.clone(),
            // A colour value is data, not prose — announced as-is.
            aria_label: v.clone(),
        })
        .collect()
}

/// Records a pick at the front of the recent list (deduplicated, capped).
pub(super) fn push_recent(mut recent: Signal<Vec<String>>, value: String) {
    let mut list = recent.peek().clone();
    list.retain(|v| v != &value);
    list.insert(0, value);
    list.truncate(RECENT_MAX);
    recent.set(list);
}

/// Shared builder: one trigger group toggling the docked panel for `target`.
fn trigger_group(
    group_label: String,
    trigger_aria: String,
    trigger_icon: &'static str,
    current_fill: Option<String>,
    target: ColorPickerTarget,
    mut open_picker: Signal<Option<ColorPickerTarget>>,
    priority: u8,
) -> RibbonGroupSpec {
    RibbonGroupSpec {
        metrics: estimate_group_metrics(priority, 1, true),
        partial: None,
        label: Some(group_label.clone()),
        aria_label: group_label,
        content: rsx! {
            AtColorPickerTrigger {
                aria_label: trigger_aria,
                current_fill: current_fill,
                is_open: open_picker() == Some(target),
                on_toggle: move |_| {
                    let next = if *open_picker.peek() == Some(target) {
                        None
                    } else {
                        Some(target)
                    };
                    open_picker.set(next);
                },
                AtIcon { path_d: trigger_icon.to_string() }
            }
        },
    }
}

/// The Font colour trigger group. `current` is the direct colour hex at the
/// caret (also the indicator fill).
pub(super) fn font_color_group(
    current: Option<String>,
    open_picker: Signal<Option<ColorPickerTarget>>,
    priority: u8,
) -> RibbonGroupSpec {
    trigger_group(
        fl!("ribbon-group-font-color"),
        fl!("ribbon-font-color-picker-aria"),
        LUCIDE_BASELINE,
        current,
        ColorPickerTarget::Text,
        open_picker,
        priority,
    )
}

/// The Highlight trigger group. `current` is the highlight **hex** at the caret
/// (also the indicator fill) — `current_highlight` resolves a named highlight to
/// its colour, so the indicator needs no lookup of its own.
pub(super) fn highlight_group(
    current: Option<String>,
    open_picker: Signal<Option<ColorPickerTarget>>,
    priority: u8,
) -> RibbonGroupSpec {
    trigger_group(
        fl!("ribbon-group-highlight"),
        fl!("ribbon-highlight-picker-aria"),
        LUCIDE_HIGHLIGHTER,
        current,
        ColorPickerTarget::Highlight,
        open_picker,
        priority,
    )
}

#[cfg(test)]
#[path = "editor_ribbon_color_tests.rs"]
mod tests;
