// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 AppThere Loki contributors

//! [`AtDialogButton`] — the actions in a dialog footer.
//!
//! # Touch targets
//!
//! At the Compact size class every button clears the 44×44 logical-pixel
//! minimum (WCAG 2.5.8), and the **primary** action is deliberately taller than
//! that floor: it is the button a user reaches for one-handed, and the extra
//! height is what separates it from Cancel by feel rather than by reading.

use dioxus::prelude::*;

use crate::tokens;

/// Compact height of the primary footer action — above [`tokens::TOUCH_MIN`] so
/// the committing action is distinguishable by feel.
const PRIMARY_TOUCH_PX: f32 = 48.0;

/// A footer action. `primary` fills; the rest are outlined or bare text.
#[component]
pub fn AtDialogButton(
    /// The localized label.
    label: String,
    /// Fill this button as the committing action.
    #[props(default = false)]
    primary: bool,
    /// Render as bare underlined text — the footer's tertiary action
    /// ("Reset all to inherited", "Clear direct formatting").
    #[props(default = false)]
    tertiary: bool,
    /// Minimum touch height (CSS px); 0 = pointer density.
    #[props(default = 0.0)]
    min_touch_px: f32,
    /// Grey the button and swallow clicks.
    #[props(default = false)]
    disabled: bool,
    /// Fired on activation.
    on_click: EventHandler<()>,
) -> Element {
    let touch_px = if min_touch_px > 0.0 && primary {
        PRIMARY_TOUCH_PX
    } else {
        min_touch_px
    };
    let touch = if touch_px > 0.0 {
        format!("min-height: {touch_px}px;")
    } else {
        String::new()
    };
    let (bg, border, fg) = if disabled {
        (
            "transparent",
            tokens::COLOR_BORDER_CHROME,
            tokens::COLOR_ICON_DISABLED,
        )
    } else if primary {
        (
            tokens::COLOR_ACCENT_PRIMARY,
            tokens::COLOR_ACCENT_PRIMARY,
            tokens::COLOR_SURFACE_PAGE,
        )
    } else if tertiary {
        (
            "transparent",
            "transparent",
            tokens::COLOR_TEXT_ON_CHROME_SECONDARY,
        )
    } else {
        (
            "transparent",
            tokens::COLOR_BORDER_CHROME,
            tokens::COLOR_TEXT_ON_CHROME,
        )
    };

    rsx! {
        button {
            disabled,
            style: format!(
                "{touch} box-sizing: border-box; display: flex; \
                 align-items: center; justify-content: center; \
                 padding: {py}px {px}px; border-radius: {r}px; \
                 cursor: {cursor}; white-space: nowrap; \
                 font-size: {fs}px; font-weight: {fw}; \
                 background: {bg}; border: 1px solid {border}; color: {fg}; {deco}",
                py = tokens::SPACE_2,
                px = if tertiary { tokens::SPACE_1 } else { tokens::SPACE_4 },
                r = tokens::RADIUS_MD,
                cursor = if disabled { "default" } else { "pointer" },
                fs = if tertiary { tokens::FONT_SIZE_META } else { tokens::FONT_SIZE_BODY },
                fw = if primary { tokens::FONT_WEIGHT_SEMIBOLD } else { tokens::FONT_WEIGHT_REGULAR },
                deco = if tertiary { "text-decoration: underline;" } else { "" },
            ),
            onclick: move |evt| {
                evt.stop_propagation();
                if !disabled {
                    on_click.call(());
                }
            },
            {label}
        }
    }
}

#[cfg(test)]
mod tests {
    /// The primary action is taller than the WCAG floor at Compact, so the
    /// committing button is distinguishable from Cancel by feel.
    #[test]
    fn primary_compact_height_exceeds_the_touch_minimum() {
        assert!(super::PRIMARY_TOUCH_PX > crate::tokens::TOUCH_MIN);
    }
}
