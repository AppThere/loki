// SPDX-License-Identifier: Apache-2.0

//! Custom-colour entry section of [`AtColorPicker`](super::AtColorPicker):
//! a colour-model selector (Hex / RGB / HSL / HSV / CMYK), the model's input
//! fields, a live preview swatch, and an Apply button.
//!
//! The model names are universal technical abbreviations, kept as named
//! constants rather than translated strings (per the design-system string
//! conventions); every prose label arrives via props.

use dioxus::prelude::*;

use super::convert::{cmyk_to_rgb, hsl_to_rgb, hsv_to_rgb, parse_hex, rgb_to_hex};
use crate::tokens;

/// The supported colour-entry models. Labels are technical abbreviations.
#[derive(Clone, Copy, PartialEq)]
enum Mode {
    Hex,
    Rgb,
    Hsl,
    Hsv,
    Cmyk,
}

const MODES: &[(Mode, &str)] = &[
    (Mode::Hex, "Hex"),
    (Mode::Rgb, "RGB"),
    (Mode::Hsl, "HSL"),
    (Mode::Hsv, "HSV"),
    (Mode::Cmyk, "CMYK"),
];

/// `(field labels, default values)` for a mode. At most four fields are used;
/// unused entries are empty.
fn fields_for(mode: Mode) -> &'static [&'static str] {
    match mode {
        Mode::Hex => &["#"],
        Mode::Rgb => &["R", "G", "B"],
        Mode::Hsl => &["H", "S", "L"],
        Mode::Hsv => &["H", "S", "V"],
        Mode::Cmyk => &["C", "M", "Y", "K"],
    }
}

/// Parses the current field values under `mode` into RGB bytes.
fn resolve(mode: Mode, f: &[String; 4]) -> Option<(u8, u8, u8)> {
    let num = |s: &String| s.trim().parse::<f32>().ok().filter(|v| v.is_finite());
    match mode {
        Mode::Hex => parse_hex(&f[0]),
        Mode::Rgb => {
            let (r, g, b) = (num(&f[0])?, num(&f[1])?, num(&f[2])?);
            let in_range = |v: f32| (0.0..=255.0).contains(&v);
            (in_range(r) && in_range(g) && in_range(b))
                .then(|| (r.round() as u8, g.round() as u8, b.round() as u8))
        }
        Mode::Hsl => Some(hsl_to_rgb(num(&f[0])?, num(&f[1])?, num(&f[2])?)),
        Mode::Hsv => Some(hsv_to_rgb(num(&f[0])?, num(&f[1])?, num(&f[2])?)),
        Mode::Cmyk => Some(cmyk_to_rgb(
            num(&f[0])?,
            num(&f[1])?,
            num(&f[2])?,
            num(&f[3])?,
        )),
    }
}

fn input_style(width_px: f32) -> String {
    format!(
        "width: {width_px}px; padding: {pv}px {ph}px; background: {bg}; \
         color: {fg}; border: 1px solid {border}; border-radius: {r}px; \
         font-family: {ff}; font-size: {fs}px;",
        pv = tokens::SPACE_1,
        ph = tokens::SPACE_1,
        bg = tokens::COLOR_SURFACE_3,
        fg = tokens::COLOR_TEXT_ON_CHROME,
        border = tokens::COLOR_BORDER_CHROME,
        r = tokens::RADIUS_SM,
        ff = tokens::FONT_FAMILY_UI,
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
) -> Element {
    let mut mode = use_signal(|| Mode::Hex);
    let mut fields = use_signal(|| [const { String::new() }; 4]);

    let resolved = resolve(mode(), &fields.read());
    let preview = resolved.map(|(r, g, b)| rgb_to_hex(r, g, b));

    let heading_style = format!(
        "font-size: {fs}px; color: {fg};",
        fs = tokens::FONT_SIZE_XS,
        fg = tokens::COLOR_TEXT_ON_CHROME_SECONDARY,
    );
    let mode_btn = |active: bool| {
        format!(
            "padding: {pv}px {ph}px; background: {bg}; color: {fg}; cursor: pointer; \
             border: 1px solid {border}; border-radius: {r}px; \
             font-family: {ff}; font-size: {fs}px;",
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
            ff = tokens::FONT_FAMILY_UI,
            fs = tokens::FONT_SIZE_XS,
        )
    };

    rsx! {
        div {
            style: format!("display: flex; flex-direction: column; gap: {}px;", tokens::SPACE_2),
            span { style: "{heading_style}", "{heading}" }

            // Colour-model selector.
            div {
                style: format!("display: flex; flex-direction: row; gap: {}px;", tokens::SPACE_1),
                for (m, label) in MODES.iter().copied() {
                    button {
                        key: "{label}",
                        style: mode_btn(mode() == m),
                        aria_pressed: if mode() == m { "true" } else { "false" },
                        onclick: move |_| {
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
                            value: "{fields.read()[i]}",
                            oninput: move |evt| { fields.write()[i] = evt.value(); },
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
                         font-family: {ff}; font-size: {fs}px;",
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
                        ff = tokens::FONT_FAMILY_UI,
                        fs = tokens::FONT_SIZE_LABEL,
                    ),
                    disabled: preview.is_none(),
                    onclick: move |_| {
                        if let Some((r, g, b)) = resolve(*mode.peek(), &fields.peek()) {
                            on_apply.call(rgb_to_hex(r, g, b));
                        }
                    },
                    "{apply_label}"
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn f(vals: [&str; 4]) -> [String; 4] {
        vals.map(str::to_string)
    }

    #[test]
    fn resolve_per_mode() {
        assert_eq!(
            resolve(Mode::Hex, &f(["#00FF00", "", "", ""])),
            Some((0, 255, 0))
        );
        assert_eq!(
            resolve(Mode::Rgb, &f(["255", "0", "0", ""])),
            Some((255, 0, 0))
        );
        assert_eq!(resolve(Mode::Rgb, &f(["300", "0", "0", ""])), None);
        assert_eq!(
            resolve(Mode::Hsl, &f(["240", "100", "50", ""])),
            Some((0, 0, 255))
        );
        assert_eq!(
            resolve(Mode::Hsv, &f(["0", "0", "100", ""])),
            Some((255, 255, 255))
        );
        assert_eq!(
            resolve(Mode::Cmyk, &f(["0", "100", "100", "0"])),
            Some((255, 0, 0))
        );
        assert_eq!(resolve(Mode::Rgb, &f(["", "0", "0", ""])), None);
        assert_eq!(resolve(Mode::Hex, &f(["not-a-color", "", "", ""])), None);
    }
}
