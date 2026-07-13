// SPDX-License-Identifier: Apache-2.0

//! Theme-variant tests (extracted; the pure `ThemeVariant` surface — the
//! Signal-backed `AtThemeContext` wrapper is exercised by component usage).

use super::*;
use crate::tokens::colors::{COLOR_SURFACE_CHROME, COLOR_TEXT_ON_CHROME};

#[test]
fn theme_variant_defaults_to_dark() {
    assert_eq!(ThemeVariant::default(), ThemeVariant::Dark);
}

#[test]
fn dark_palette_is_the_token_constants() {
    let p = ThemeVariant::Dark.palette();
    assert_eq!(p.surface_chrome, COLOR_SURFACE_CHROME);
    assert_eq!(p.text_on_chrome, COLOR_TEXT_ON_CHROME);
}

#[test]
fn light_palette_re_colors_the_chrome() {
    let dark = ThemeVariant::Dark.palette();
    let light = ThemeVariant::Light.palette();
    assert_ne!(light.surface_chrome, dark.surface_chrome);
    assert_ne!(light.text_on_chrome, dark.text_on_chrome);
    // The document page stays white in both variants.
    assert_eq!(light.surface_page, dark.surface_page);
}

#[test]
fn toggled_flips_and_round_trips() {
    assert_eq!(ThemeVariant::Dark.toggled(), ThemeVariant::Light);
    assert_eq!(ThemeVariant::Light.toggled(), ThemeVariant::Dark);
    assert_eq!(ThemeVariant::Dark.toggled().toggled(), ThemeVariant::Dark);
}
