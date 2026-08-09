// SPDX-License-Identifier: Apache-2.0

//! Custom-colour entry section of [`AtColorPicker`](super::AtColorPicker):
//! a colour-model selector (Hex / RGB / HSL / HSV / CMYK), the model's input
//! fields, a live preview swatch, and an Apply button.
//!
//! The model names are universal technical abbreviations, kept as named
//! constants rather than translated strings (per the design-system string
//! conventions); every prose label arrives via props.

use dioxus::prelude::*;

use super::area::AtColorArea;
use super::convert::{hsv_to_rgb, rgb_to_hex};
use super::custom_mode::{field_mode, fields_for, resolve, Mode, MODES};
use super::custom_source::{displayed_hsv, fields_from_rgb, Source};
use crate::tokens;

fn input_style(width_px: f32) -> String {
    format!(
        "width: {width_px}px; padding: {pv}px {ph}px; background: {bg}; \
         color: {fg}; border: 1px solid {border}; border-radius: {r}px; \
         font-size: {fs}px;",
        pv = tokens::SPACE_1,
        ph = tokens::SPACE_1,
        bg = tokens::COLOR_SURFACE_3,
        fg = tokens::COLOR_TEXT_ON_CHROME,
        border = tokens::COLOR_BORDER_CHROME,
        r = tokens::RADIUS_SM,
        fs = tokens::FONT_SIZE_LABEL,
    )
}

/// The custom-colour entry section.
///
/// # Touch target
///
/// The mode buttons and Apply button are text buttons with padded hit areas;
/// the section sits inside the picker popover whose row heights keep each
/// interactive element within a ≥44×44 logical-pixel touch target (WCAG 2.5.8).
#[component]
pub(super) fn CustomColorSection(
    /// Section heading (translated), e.g. "Custom".
    heading: String,
    /// Apply button label (translated).
    apply_label: String,
    /// Called with the resolved `#RRGGBB` when Apply is pressed.
    on_apply: EventHandler<String>,
    /// Accessible name of the saturation/value square.
    area_label: String,
    /// Accessible name of the hue strip.
    hue_label: String,
) -> Element {
    let mut mode = use_signal(|| Mode::Hex);
    let mut fields = use_signal(|| [const { String::new() }; 4]);
    // The square's own colour, and which control last spoke — see
    // `custom_source` for why "last edited wins" rather than writing back.
    let mut area_hsv = use_signal(|| (0.0_f32, 100.0_f32, 100.0_f32));
    let mut source = use_signal(Source::default);

    let field_rgb = resolve(mode(), &fields.read());
    let shown_hsv = displayed_hsv(source(), area_hsv(), field_rgb);
    // One displayed colour, whichever control produced it. The area is always
    // resolvable, so dragging never leaves the section without a preview — the
    // fields can be mid-edit and the square cannot.
    let resolved = match source() {
        Source::Area => Some(hsv_to_rgb(shown_hsv.0, shown_hsv.1, shown_hsv.2)),
        Source::Fields => field_rgb,
    };
    let preview = resolved.map(|(r, g, b)| rgb_to_hex(r, g, b));
    // What the fields *show*. Derived from the square while the square is the
    // source, so a dragged colour has a readable hex to copy; the reader's own
    // text the moment they type. See `custom_source::fields_from_rgb`.
    let shown_fields = match (source(), resolved) {
        (Source::Area, Some(rgb)) => fields_from_rgb(field_mode(mode()), rgb),
        _ => fields.read().clone(),
    };

    let heading_style = format!(
        "font-size: {fs}px; color: {fg};",
        fs = tokens::FONT_SIZE_XS,
        fg = tokens::COLOR_TEXT_ON_CHROME_SECONDARY,
    );
    let mode_btn = |active: bool| {
        format!(
            "padding: {pv}px {ph}px; background: {bg}; color: {fg}; cursor: pointer; \
             border: 1px solid {border}; border-radius: {r}px; \
             font-size: {fs}px;",
            pv = tokens::SPACE_1,
            ph = tokens::SPACE_1,
            bg = if active {
                tokens::COLOR_TAB_ACTIVE_BG
            } else {
                "transparent"
            },
            fg = if active {
                tokens::COLOR_TEXT_ACCENT
            } else {
                tokens::COLOR_TEXT_ON_CHROME
            },
            border = if active {
                tokens::COLOR_TAB_ACTIVE_INDICATOR
            } else {
                tokens::COLOR_BORDER_CHROME
            },
            r = tokens::RADIUS_SM,
            fs = tokens::FONT_SIZE_XS,
        )
    };

    rsx! {
        div {
            style: format!("display: flex; flex-direction: column; gap: {}px;", tokens::SPACE_2),
            span { style: "{heading_style}", "{heading}" }

            // The saturation/value square and hue strip (T5.2). Above the
            // fields, because it is how most people pick a colour and the
            // fields are how they refine or paste one.
            AtColorArea {
                hue: shown_hsv.0,
                saturation: shown_hsv.1,
                value: shown_hsv.2,
                area_label: area_label.clone(),
                hue_label: hue_label.clone(),
                on_change: move |(h, s, v): (f32, f32, f32)| {
                    area_hsv.set((h, s, v));
                    source.set(Source::Area);
                },
            }

            // Colour-model selector.
            div {
                style: format!("display: flex; flex-direction: row; gap: {}px;", tokens::SPACE_1),
                for (m, label) in MODES.iter().copied() {
                    button {
                        key: "{label}",
                        style: mode_btn(mode() == m),
                        aria_pressed: if mode() == m { "true" } else { "false" },
                        onclick: move |_| {
                            source.set(Source::Fields);
                            if *mode.peek() != m {
                                mode.set(m);
                                fields.set([const { String::new() }; 4]);
                            }
                        },
                        "{label}"
                    }
                }
            }

            // The active model's input fields.
            div {
                style: format!(
                    "display: flex; flex-direction: row; align-items: center; gap: {}px;",
                    tokens::SPACE_1,
                ),
                for (i, label) in fields_for(mode()).iter().copied().enumerate() {
                    div {
                        key: "{label}",
                        style: format!(
                            "display: flex; flex-direction: row; align-items: center; gap: {}px;",
                            tokens::SPACE_1,
                        ),
                        span { style: "{heading_style}", "{label}" }
                        input {
                            r#type: "text",
                            value: "{shown_fields[i]}",
                            oninput: move |evt| {
                                // Typing hands the colour back to the fields —
                                // "last edited wins" (see `custom_source`).
                                source.set(Source::Fields);
                                fields.write()[i] = evt.value();
                            },
                            style: input_style(if mode() == Mode::Hex { 96.0 } else { 40.0 }),
                        }
                    }
                }
            }

            // Live preview + Apply.
            div {
                style: format!(
                    "display: flex; flex-direction: row; align-items: center; gap: {}px;",
                    tokens::SPACE_2,
                ),
                div {
                    style: format!(
                        "width: 22px; height: 22px; border-radius: {r}px; background: {bg}; \
                         border: 1px solid {border};",
                        r = tokens::RADIUS_SM,
                        bg = preview.clone().unwrap_or_else(|| "transparent".to_string()),
                        border = tokens::COLOR_BORDER_CHROME,
                    ),
                }
                button {
                    style: format!(
                        "padding: {pv}px {ph}px; background: {bg}; color: {fg}; \
                         border: 1px solid {border}; border-radius: {r}px; cursor: pointer; \
                         font-size: {fs}px;",
                        pv = tokens::SPACE_1,
                        ph = tokens::SPACE_3,
                        bg = tokens::COLOR_SURFACE_3,
                        fg = if preview.is_some() {
                            tokens::COLOR_TEXT_ON_CHROME
                        } else {
                            tokens::COLOR_ICON_DISABLED
                        },
                        border = tokens::COLOR_BORDER_CHROME,
                        r = tokens::RADIUS_SM,
                        fs = tokens::FONT_SIZE_LABEL,
                    ),
                    disabled: preview.is_none(),
                    // **The colour applied is the one on the swatch beside it** —
                    // the same `preview` the line above gates on. It read
                    // `resolve(mode, fields)` until T5.3, which is the *typed*
                    // colour and empty while the square is the source, so the
                    // button was enabled by one question and acted on another:
                    // dragging and pressing Apply did nothing at all. Found by a
                    // screen sitting; see `the_typed_fields_do_not_speak_for_a_
                    // dragged_colour`.
                    onclick: {
                        let hex = preview.clone();
                        move |_| {
                            if let Some(hex) = hex.clone() {
                                on_apply.call(hex);
                            }
                        }
                    },
                    "{apply_label}"
                }
            }
        }
    }
}
