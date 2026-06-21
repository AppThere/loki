// SPDX-License-Identifier: Apache-2.0

//! Theme context for AppThere shell components.
//!
//! Inject at the application root with `provide_context(AtThemeContext::default())`.
//! Read in any descendant component with [`use_theme`].

use dioxus::prelude::*;

// ── ThemeVariant ──────────────────────────────────────────────────────────────

/// Selects which color palette the shell components use.
#[derive(Clone, Copy, PartialEq, Eq, Default, Debug)]
pub enum ThemeVariant {
    /// Dark shell chrome (default — the only implemented variant).
    #[default]
    Dark,
    // TODO(theme): Light theme tokens are not yet implemented.
    /// Light color palette (not yet implemented).
    Light,
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
#[derive(Clone, PartialEq, Debug)]
pub struct AtThemeContext {
    /// Active color palette variant.
    pub variant: ThemeVariant,
}

impl Default for AtThemeContext {
    fn default() -> Self {
        Self {
            variant: ThemeVariant::Dark,
        }
    }
}

// ── use_theme ─────────────────────────────────────────────────────────────────

/// Reads the [`AtThemeContext`] injected at the application root.
///
/// # Panics
///
/// Panics in debug builds if [`AtThemeContext`] has not been provided via
/// `provide_context(AtThemeContext::default())` in an ancestor component.
pub fn use_theme() -> AtThemeContext {
    use_context::<AtThemeContext>()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn theme_variant_defaults_to_dark() {
        assert_eq!(ThemeVariant::default(), ThemeVariant::Dark);
    }

    #[test]
    fn theme_context_defaults_to_dark_variant() {
        assert_eq!(AtThemeContext::default().variant, ThemeVariant::Dark);
    }
}
