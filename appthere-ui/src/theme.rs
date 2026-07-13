// SPDX-License-Identifier: Apache-2.0

//! Theme context for AppThere shell components.
//!
//! Inject at the application root with `provide_context(AtThemeContext::default())`.
//! Read in any descendant component with [`use_theme`]; resolve colors with
//! [`AtThemeContext::palette`]. The variant is Signal-backed, so a component
//! that reads `palette()` (or `variant()`) inside its render re-colors live
//! when the variant changes (e.g. via [`AtThemeContext::toggle`]).

use dioxus::prelude::*;

use crate::tokens::ThemePalette;

// ── ThemeVariant ──────────────────────────────────────────────────────────────

/// Selects which color palette the shell components use.
#[derive(Clone, Copy, PartialEq, Eq, Default, Debug)]
pub enum ThemeVariant {
    /// Dark shell chrome (default).
    #[default]
    Dark,
    /// Light shell chrome ([`ThemePalette::light`]).
    Light,
}

impl ThemeVariant {
    /// The color palette for this variant (pure — the reactive read lives on
    /// [`AtThemeContext::palette`]).
    #[must_use]
    pub const fn palette(self) -> ThemePalette {
        match self {
            Self::Dark => ThemePalette::dark(),
            Self::Light => ThemePalette::light(),
        }
    }

    /// The other variant (Dark ⇄ Light).
    #[must_use]
    pub const fn toggled(self) -> Self {
        match self {
            Self::Dark => Self::Light,
            Self::Light => Self::Dark,
        }
    }
}

// ── AtThemeContext ────────────────────────────────────────────────────────────

/// Theme context injected at the application root and read by shell components.
///
/// # Usage
///
/// At the app root (before any shell component renders):
/// ```rust,ignore
/// provide_context(AtThemeContext::default()); // ThemeVariant::Dark
/// ```
#[derive(Clone, Copy, PartialEq, Debug)]
pub struct AtThemeContext {
    /// Active color palette variant (Signal-backed for live re-theming).
    pub variant: Signal<ThemeVariant>,
}

impl AtThemeContext {
    /// Creates a context with the given starting variant. Must be called in
    /// a reactive scope (a component body), like `Signal::new`.
    #[must_use]
    pub fn new(variant: ThemeVariant) -> Self {
        Self {
            variant: Signal::new(variant),
        }
    }

    /// The active variant (reactive read).
    #[must_use]
    pub fn variant(&self) -> ThemeVariant {
        (self.variant)()
    }

    /// The color palette for the active variant (reactive read).
    #[must_use]
    pub fn palette(&self) -> ThemePalette {
        self.variant().palette()
    }

    /// Flips Dark ⇄ Light. Every component reading [`Self::palette`] in its
    /// render re-colors.
    pub fn toggle(&mut self) {
        let next = self.variant().toggled();
        self.variant.set(next);
    }
}

impl Default for AtThemeContext {
    fn default() -> Self {
        Self::new(ThemeVariant::Dark)
    }
}

// ── use_theme ─────────────────────────────────────────────────────────────────

/// Reads the [`AtThemeContext`] injected at the application root, falling
/// back to a component-local Dark-variant context when none was provided
/// (so a component tree without a themed root — tests, embedding — still
/// renders; the `use_breakpoint` resilience pattern).
#[must_use]
pub fn use_theme() -> AtThemeContext {
    use_hook(|| try_consume_context::<AtThemeContext>().unwrap_or_default())
}

#[cfg(test)]
#[path = "theme_tests.rs"]
mod tests;
