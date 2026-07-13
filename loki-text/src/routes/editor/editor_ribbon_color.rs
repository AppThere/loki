// SPDX-License-Identifier: Apache-2.0

//! Format-tab colour-picker trigger groups (**Font colour** / **Highlight**)
//! and the shared colour palettes.
//!
//! Each group is a single [`AtColorPickerTrigger`] whose click toggles the
//! docked picker panel (`editor_color_panel`) above the ribbon — the ribbon
//! content row is a scroll container, so the panel cannot be anchored inside
//! it (see the `appthere_ui` colour-picker module docs).
//!
//! Highlight is constrained to the named `HighlightColor` palette because the
//! document model (and DOCX `w:highlight`) only round-trips named variants.
//! TODO(highlight-custom-color): offering arbitrary hex highlight needs a
//! `HighlightColor` model extension plus OOXML (`w:shd`) / ODF export mapping.

use appthere_ui::{
    AtColorPickerTrigger, AtColorSwatch, AtIcon, LUCIDE_BASELINE, LUCIDE_HIGHLIGHTER,
    RibbonGroupSpec, estimate_group_metrics,
};
use dioxus::prelude::*;
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

/// The full named highlight palette as `(variant name, fill, aria-key)`. The
/// fills mirror `loki-layout`'s `map_highlight_color` so the swatch matches
/// what the page paints.
pub(super) const HIGHLIGHT_PALETTE: &[(&str, &str, &str)] = &[
    ("Yellow", "#FFFF00", "ribbon-highlight-yellow-aria"),
    ("Green", "#00FF00", "ribbon-highlight-green-aria"),
    ("Cyan", "#00FFFF", "ribbon-highlight-cyan-aria"),
    ("Magenta", "#FF00FF", "ribbon-highlight-magenta-aria"),
    ("Blue", "#0000FF", "ribbon-highlight-blue-aria"),
    ("Red", "#FF0000", "ribbon-highlight-red-aria"),
    ("DarkBlue", "#000080", "ribbon-highlight-dark-blue-aria"),
    ("DarkCyan", "#008080", "ribbon-highlight-dark-cyan-aria"),
    ("DarkGreen", "#008000", "ribbon-highlight-dark-green-aria"),
    (
        "DarkMagenta",
        "#800080",
        "ribbon-highlight-dark-magenta-aria",
    ),
    ("DarkRed", "#800000", "ribbon-highlight-dark-red-aria"),
    ("DarkYellow", "#808000", "ribbon-highlight-dark-yellow-aria"),
    ("DarkGray", "#808080", "ribbon-highlight-dark-gray-aria"),
    ("LightGray", "#C0C0C0", "ribbon-highlight-light-gray-aria"),
    ("Black", "#000000", "ribbon-highlight-black-aria"),
    ("White", "#FFFFFF", "ribbon-highlight-white-aria"),
];

/// The display fill for a named highlight value, if known.
pub(super) fn highlight_fill(name: &str) -> Option<&'static str> {
    HIGHLIGHT_PALETTE
        .iter()
        .find(|(value, _, _)| *value == name)
        .map(|(_, fill, _)| *fill)
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

/// Recent values → swatches. `fill_for` maps a stored value to its display
/// fill (identity for hex text colours, the palette lookup for highlights).
pub(super) fn recent_swatches(
    values: &[String],
    fill_for: fn(&str) -> String,
) -> Vec<AtColorSwatch> {
    values
        .iter()
        .map(|v| AtColorSwatch {
            value: v.clone(),
            fill: fill_for(v),
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

/// The Highlight trigger group. `current` is the named highlight at the caret.
pub(super) fn highlight_group(
    current: Option<String>,
    open_picker: Signal<Option<ColorPickerTarget>>,
    priority: u8,
) -> RibbonGroupSpec {
    let current_fill = current
        .as_deref()
        .and_then(highlight_fill)
        .map(str::to_string);
    trigger_group(
        fl!("ribbon-group-highlight"),
        fl!("ribbon-highlight-picker-aria"),
        LUCIDE_HIGHLIGHTER,
        current_fill,
        ColorPickerTarget::Highlight,
        open_picker,
        priority,
    )
}
