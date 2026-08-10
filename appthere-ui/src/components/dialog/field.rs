// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 AppThere Loki contributors

//! Field scaffolding shared by every Loki dialog: the uppercase caption above a
//! control, the disabled-with-a-reason treatment, and the `※` notice card.

use dioxus::prelude::*;

use crate::tokens;

/// The uppercase caption that sits above a control.
///
/// Returned as a style string rather than a component so a caller can put it on
/// whatever element the layout needs (a `div` in a column, a `span` in a row).
#[must_use]
pub fn at_field_label_style() -> String {
    format!(
        "font-size: {fs}px; font-weight: {fw}; letter-spacing: 0.04em; \
         text-transform: uppercase; color: {fg};",
        fs = tokens::FONT_SIZE_LABEL,
        fw = tokens::FONT_WEIGHT_SEMIBOLD,
        fg = tokens::COLOR_TEXT_ON_CHROME_SECONDARY,
    )
}

/// The shared box style for an inline control (a value chip, a select-alike, a
/// stepper). `min_touch_px` comes from the dialog posture; `extra` injects
/// per-call-site width or flex rules.
#[must_use]
pub fn at_control_style(min_touch_px: f32, extra: &str) -> String {
    let touch = if min_touch_px > 0.0 {
        format!("min-height: {min_touch_px}px;")
    } else {
        String::new()
    };
    format!(
        "{extra} {touch} box-sizing: border-box; display: flex; \
         flex-direction: row; align-items: center; justify-content: space-between; \
         gap: {gap}px; padding: {py}px {px}px; background: {bg}; \
         border: 1px solid {border}; border-radius: {r}px; \
         font-size: {fs}px; color: {fg};",
        gap = tokens::SPACE_2,
        py = tokens::SPACE_2,
        px = tokens::SPACE_3,
        bg = tokens::COLOR_SURFACE_2,
        border = tokens::COLOR_BORDER_CHROME,
        r = tokens::RADIUS_MD,
        fs = tokens::FONT_SIZE_MD,
        fg = tokens::COLOR_TEXT_ON_CHROME,
    )
}

/// A labelled field: caption, control, and an optional line beneath it
/// (provenance, a unit hint, or a disabled reason).
///
/// # Inapplicable controls stay visible
///
/// A control that does not apply in the current state is rendered `disabled`
/// with its reason in `footnote` rather than hidden. A form whose fields appear
/// and vanish as neighbouring values change cannot be learnt, and a user
/// hunting for a control they saw yesterday has no way to tell "not applicable"
/// from "moved".
#[component]
pub fn AtField(
    /// Localized caption. `None` renders the control with no caption row.
    #[props(default)]
    label: Option<String>,
    /// The control itself.
    control: Element,
    /// The line beneath the control — usually an `AtProvenanceLine`.
    #[props(default)]
    footnote: Option<Element>,
    /// Grey the caption to signal the whole field is inapplicable.
    #[props(default = false)]
    disabled: bool,
    /// Extra rules on the field's own column (e.g. `grid-column: 1 / -1;`).
    #[props(default = String::new())]
    extra_style: String,
) -> Element {
    let caption_fg = if disabled {
        tokens::COLOR_ICON_DISABLED
    } else {
        tokens::COLOR_TEXT_ON_CHROME_SECONDARY
    };
    rsx! {
        div {
            style: format!(
                "display: flex; flex-direction: column; gap: {gap}px; min-width: 0; {extra_style}",
                gap = tokens::SPACE_1,
            ),
            if let Some(text) = label {
                div {
                    style: format!("{} color: {caption_fg};", at_field_label_style()),
                    {text}
                }
            }
            {control}
            if let Some(note) = footnote {
                {note}
            }
        }
    }
}

/// Which voice a [`AtDialogNotice`] speaks in.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum AtNoticeTone {
    /// Neutral explanation — how a value resolves, what a control means.
    Info,
    /// A consequence the user should see before committing.
    Caution,
    /// A check that passed.
    Positive,
}

impl AtNoticeTone {
    /// The leading glyph. `※` marks a consequence, `↳` a plain explanation, and
    /// `✓` a passed check — all single glyphs present in the bundled UI face, so
    /// the tone survives without colour on E-Ink.
    #[must_use]
    fn marker(self) -> &'static str {
        match self {
            AtNoticeTone::Info => "\u{21B3}",
            AtNoticeTone::Caution => "\u{203B}",
            AtNoticeTone::Positive => "\u{2713}",
        }
    }

    /// The marker's colour.
    #[must_use]
    fn marker_color(self) -> &'static str {
        match self {
            AtNoticeTone::Info => tokens::COLOR_TEXT_ON_CHROME_SECONDARY,
            AtNoticeTone::Caution => tokens::COLOR_CONTEXTUAL_TAB,
            AtNoticeTone::Positive => tokens::COLOR_TEXT_ACCENT,
        }
    }
}

/// The inset card that states a consequence — how many properties a re-parent
/// re-resolves, which sections a page style touches, what an export will drop.
#[component]
pub fn AtDialogNotice(
    /// The notice's voice.
    #[props(default = AtNoticeTone::Info)]
    tone: AtNoticeTone,
    /// The already-localized body.
    message: Element,
    /// Extra rules on the card (e.g. `grid-column: 1 / -1;`).
    #[props(default = String::new())]
    extra_style: String,
) -> Element {
    rsx! {
        div {
            style: format!(
                "display: flex; flex-direction: row; align-items: flex-start; \
                 gap: {gap}px; padding: {py}px {px}px; background: {bg}; \
                 border: 1px solid {border}; border-radius: {r}px; \
                 font-size: {fs}px; color: {fg}; {extra_style}",
                gap = tokens::SPACE_2,
                py = tokens::SPACE_3,
                px = tokens::SPACE_3,
                bg = tokens::COLOR_SURFACE_2,
                border = tokens::COLOR_BORDER_CHROME,
                r = tokens::RADIUS_MD,
                fs = tokens::FONT_SIZE_META,
                fg = tokens::COLOR_TEXT_ON_CHROME,
            ),
            span {
                style: format!("flex-shrink: 0; color: {};", tone.marker_color()),
                {tone.marker()}
            }
            div { {message} }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Tone must be legible without colour (E-Ink, 1-bit), so the three markers
    /// are pairwise distinct glyphs rather than one glyph in three colours.
    #[test]
    fn every_notice_tone_has_a_distinct_marker() {
        let markers = [
            AtNoticeTone::Info.marker(),
            AtNoticeTone::Caution.marker(),
            AtNoticeTone::Positive.marker(),
        ];
        for (i, a) in markers.iter().enumerate() {
            for b in &markers[i + 1..] {
                assert_ne!(a, b);
            }
        }
    }

    /// A touch posture must emit the floor; pointer density must not — a helper
    /// that always emitted `min-height` would silently inflate dense forms.
    #[test]
    fn control_style_emits_a_touch_floor_only_when_asked() {
        assert!(at_control_style(tokens::TOUCH_MIN, "").contains("min-height: 44px"));
        assert!(!at_control_style(0.0, "").contains("min-height"));
    }

    #[test]
    fn control_style_passes_through_call_site_rules() {
        assert!(at_control_style(0.0, "flex: 1;").contains("flex: 1;"));
    }
}
