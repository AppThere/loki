// SPDX-License-Identifier: Apache-2.0

//! Variant-resolved color palette (deferred-features 4c.4, topic `theme`).
//!
//! [`ThemePalette`] carries one field per color token in
//! [`colors`](super::colors), so a component that reads
//! `use_theme().palette()` re-colors with the active [`ThemeVariant`]
//! (`crate::theme`). The `COLOR_*` constants remain the **dark** values, so
//! unmigrated call sites keep compiling and rendering exactly as before;
//! components migrate to the palette opportunistically, shell chrome first.
//!
//! [`ThemePalette::dark`] is built *from* the constants (zero drift by
//! construction); [`ThemePalette::light`] is the inverted set. Tokens that
//! are surface-independent (the white page, error banner, disabled opacity)
//! keep the same value in both variants.

use super::colors::*;

/// One resolved color per [`colors`](super::colors) token. All values are
/// CSS color strings suitable for inline `style` attributes.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
#[allow(missing_docs)] // Field names mirror the documented `COLOR_*` tokens.
pub struct ThemePalette {
    pub surface_page: &'static str,
    pub surface_base: &'static str,
    pub surface_chrome: &'static str,
    pub surface_1: &'static str,
    pub surface_2: &'static str,
    pub surface_3: &'static str,
    pub border_default: &'static str,
    pub border_chrome: &'static str,
    pub accent_primary: &'static str,
    pub accent_primary_hover: &'static str,
    pub text_primary: &'static str,
    pub text_secondary: &'static str,
    pub text_on_chrome: &'static str,
    pub text_on_chrome_secondary: &'static str,
    pub text_accent: &'static str,
    pub tab_active_bg: &'static str,
    pub tab_active_indicator: &'static str,
    pub tab_inactive_hover: &'static str,
    pub contextual_tab: &'static str,
    pub icon_disabled: &'static str,
    pub canvas_page_bg: &'static str,
    pub canvas_margin_bg: &'static str,
    pub scrollbar_thumb: &'static str,
    pub scrollbar_thumb_hover: &'static str,
    pub scrollbar_track: &'static str,
    pub status_error_bg: &'static str,
    pub status_error_text: &'static str,
    pub status_error_border: &'static str,
}

impl ThemePalette {
    /// The dark palette — exactly the `COLOR_*` constants, by construction.
    #[must_use]
    pub const fn dark() -> Self {
        Self {
            surface_page: COLOR_SURFACE_PAGE,
            surface_base: COLOR_SURFACE_BASE,
            surface_chrome: COLOR_SURFACE_CHROME,
            surface_1: COLOR_SURFACE_1,
            surface_2: COLOR_SURFACE_2,
            surface_3: COLOR_SURFACE_3,
            border_default: COLOR_BORDER_DEFAULT,
            border_chrome: COLOR_BORDER_CHROME,
            accent_primary: COLOR_ACCENT_PRIMARY,
            accent_primary_hover: COLOR_ACCENT_PRIMARY_HOVER,
            text_primary: COLOR_TEXT_PRIMARY,
            text_secondary: COLOR_TEXT_SECONDARY,
            text_on_chrome: COLOR_TEXT_ON_CHROME,
            text_on_chrome_secondary: COLOR_TEXT_ON_CHROME_SECONDARY,
            text_accent: COLOR_TEXT_ACCENT,
            tab_active_bg: COLOR_TAB_ACTIVE_BG,
            tab_active_indicator: COLOR_TAB_ACTIVE_INDICATOR,
            tab_inactive_hover: COLOR_TAB_INACTIVE_HOVER,
            contextual_tab: COLOR_CONTEXTUAL_TAB,
            icon_disabled: COLOR_ICON_DISABLED,
            canvas_page_bg: CANVAS_PAGE_BG,
            canvas_margin_bg: CANVAS_MARGIN_BG,
            scrollbar_thumb: COLOR_SCROLLBAR_THUMB,
            scrollbar_thumb_hover: COLOR_SCROLLBAR_THUMB_HOVER,
            scrollbar_track: COLOR_SCROLLBAR_TRACK,
            status_error_bg: COLOR_STATUS_ERROR_BG,
            status_error_text: COLOR_STATUS_ERROR_TEXT,
            status_error_border: COLOR_STATUS_ERROR_BORDER,
        }
    }

    /// The light palette. Chrome surfaces invert to light grays with dark
    /// text; the document page, error banner, and page-side tokens keep
    /// their values. Accents darken slightly to hold contrast on light
    /// backgrounds (WCAG-minded, flat-design aesthetic preserved).
    #[must_use]
    pub const fn light() -> Self {
        Self {
            surface_page: COLOR_SURFACE_PAGE,
            surface_base: "#C9C9C9",
            surface_chrome: "#F3F3F3",
            surface_1: "#ECECEC",
            surface_2: "#E4E4E4",
            surface_3: "#DDDDDD",
            border_default: COLOR_BORDER_DEFAULT,
            border_chrome: "#CFCFCF",
            accent_primary: "#2B6BE6",
            accent_primary_hover: "#2560D0",
            text_primary: COLOR_TEXT_PRIMARY,
            text_secondary: COLOR_TEXT_SECONDARY,
            text_on_chrome: "#1F1F1F",
            text_on_chrome_secondary: "#5A5A5A",
            text_accent: "#1F5FD6",
            tab_active_bg: "#FFFFFF",
            tab_active_indicator: "#2B6BE6",
            tab_inactive_hover: "#E2E2E2",
            contextual_tab: "#B87A10",
            icon_disabled: "#ABABAB",
            canvas_page_bg: CANVAS_PAGE_BG,
            canvas_margin_bg: "#B4B4B4",
            scrollbar_thumb: "rgba(0, 0, 0, 0.25)",
            scrollbar_thumb_hover: "rgba(0, 0, 0, 0.50)",
            scrollbar_track: "rgba(0, 0, 0, 0.06)",
            status_error_bg: COLOR_STATUS_ERROR_BG,
            status_error_text: COLOR_STATUS_ERROR_TEXT,
            status_error_border: COLOR_STATUS_ERROR_BORDER,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn chrome_fields(p: &ThemePalette) -> [(&'static str, &'static str); 17] {
        [
            ("surface_base", p.surface_base),
            ("surface_chrome", p.surface_chrome),
            ("surface_1", p.surface_1),
            ("surface_2", p.surface_2),
            ("surface_3", p.surface_3),
            ("border_chrome", p.border_chrome),
            ("accent_primary", p.accent_primary),
            ("accent_primary_hover", p.accent_primary_hover),
            ("text_on_chrome", p.text_on_chrome),
            ("text_on_chrome_secondary", p.text_on_chrome_secondary),
            ("text_accent", p.text_accent),
            ("tab_active_bg", p.tab_active_bg),
            ("tab_active_indicator", p.tab_active_indicator),
            ("tab_inactive_hover", p.tab_inactive_hover),
            ("contextual_tab", p.contextual_tab),
            ("icon_disabled", p.icon_disabled),
            ("canvas_margin_bg", p.canvas_margin_bg),
        ]
    }

    #[test]
    fn dark_palette_matches_the_token_constants() {
        let d = ThemePalette::dark();
        assert_eq!(d.surface_chrome, COLOR_SURFACE_CHROME);
        assert_eq!(d.text_on_chrome, COLOR_TEXT_ON_CHROME);
        assert_eq!(d.tab_active_bg, COLOR_TAB_ACTIVE_BG);
        assert_eq!(d.scrollbar_thumb, COLOR_SCROLLBAR_THUMB);
    }

    #[test]
    fn light_differs_from_dark_on_every_chrome_token() {
        let dark = ThemePalette::dark();
        let light = ThemePalette::light();
        for ((name, d), (_, l)) in chrome_fields(&dark).iter().zip(chrome_fields(&light)) {
            assert_ne!(*d, l, "chrome token {name} must re-color in light");
        }
    }

    #[test]
    fn surface_independent_tokens_are_shared() {
        let dark = ThemePalette::dark();
        let light = ThemePalette::light();
        assert_eq!(dark.surface_page, light.surface_page);
        assert_eq!(dark.status_error_bg, light.status_error_bg);
        assert_eq!(dark.text_primary, light.text_primary);
    }

    #[test]
    fn light_hex_tokens_are_well_formed() {
        let is_hex6 = |s: &str| {
            s.len() == 7 && s.starts_with('#') && s[1..].chars().all(|c| c.is_ascii_hexdigit())
        };
        let l = ThemePalette::light();
        for (name, v) in chrome_fields(&l) {
            if v.starts_with('#') {
                assert!(is_hex6(v), "light token {name} malformed: {v}");
            }
        }
        for v in [
            l.scrollbar_thumb,
            l.scrollbar_thumb_hover,
            l.scrollbar_track,
        ] {
            assert!(v.starts_with("rgba("), "scrollbar tokens stay rgba: {v}");
        }
    }
}
