// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 AppThere Loki contributors

//! [`AtProvenanceLine`] — the line under a control saying where its value comes
//! from (design note 01).
//!
//! # Why this is a line and not a tooltip
//!
//! Provenance sits **under every control, always visible**. It must survive on
//! E-Ink and on touch, where there is no hover, and it must be readable in a
//! screenshot — so it cannot be a tooltip, a hover chip, or an icon whose
//! meaning has to be learnt. An inherited row reads `↳ source · value`; a local
//! override gets the accent dot, the word for "set here", and a Reset.
//!
//! # This component is model-agnostic
//!
//! `appthere_ui` must not know about `StyleCatalog`, so the caller maps its own
//! provenance type to [`AtProvenanceKind`] and passes **already-localized**
//! strings (crate convention: components are i18n-agnostic). In Loki Text that
//! mapping lives in `routes::editor::style_dialog::rows`.
//!
//! # Touch targets
//!
//! The Reset and jump affordances are buttons. Pass `min_touch_px` from
//! [`super::DialogPosture::min_touch_px`] so they reach the 44×44 logical-pixel
//! minimum (WCAG 2.5.8) at the Compact size class; at pointer density they stay
//! inline-sized so a dense form does not gain 44 px per row.

use dioxus::prelude::*;

use crate::tokens;

/// Where a value comes from — the four levels a resolved property can carry.
///
/// Span-level (direct) formatting is a *fifth* source that sits above every
/// style (design note 09). It maps onto [`AtProvenanceKind::Local`], which is
/// the level "Clear direct formatting" removes; the caller supplies the wording
/// that distinguishes them.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum AtProvenanceKind {
    /// Set on the inspected object itself — an override, and resettable.
    Local,
    /// Resolved from a named ancestor.
    Inherited,
    /// Supplied by the document default.
    Default,
    /// Unset everywhere; the layout engine supplies the value.
    Engine,
}

impl AtProvenanceKind {
    /// `true` for the one kind that owns its value and can be reset.
    #[must_use]
    pub fn is_local(self) -> bool {
        matches!(self, AtProvenanceKind::Local)
    }

    /// The leading glyph: an accent dot for a local override, the inheritance
    /// hook otherwise.
    ///
    /// U+21B3 (↳) is deliberate — it is one glyph, present in the bundled UI
    /// face, and reads as "comes from" without colour, which is what makes the
    /// line work at 1-bit on E-Ink.
    #[must_use]
    pub fn marker(self) -> &'static str {
        if self.is_local() {
            "\u{25CF}"
        } else {
            "\u{21B3}"
        }
    }
}

/// The line under a control naming the source of its value.
///
/// `text` is the whole already-localized sentence (`Inherited from Body · 12 pt`,
/// `Document default · automatic`). `reset_label` / `on_reset` render the reset
/// affordance — supplied only for locally-set rows. `jump_label` / `on_jump`
/// render the "edit at the source" link, which inherited rows carry so a user
/// can change the value where it is actually set (design note 02).
#[component]
pub fn AtProvenanceLine(
    /// Which level the value resolved from.
    kind: AtProvenanceKind,
    /// The localized provenance sentence, without the leading marker.
    text: String,
    /// Localized label for the reset control; `None` hides it.
    #[props(default)]
    reset_label: Option<String>,
    /// Fired when the user resets the local override.
    #[props(default)]
    on_reset: Option<EventHandler<()>>,
    /// Localized label for the jump-to-source link; `None` hides it.
    #[props(default)]
    jump_label: Option<String>,
    /// Fired when the user follows the jump-to-source link.
    #[props(default)]
    on_jump: Option<EventHandler<()>>,
    /// Minimum touch height (CSS px) for the affordances; 0 = pointer density.
    #[props(default = 0.0)]
    min_touch_px: f32,
) -> Element {
    let accent = kind.is_local();
    let fg = if accent {
        tokens::COLOR_TAB_ACTIVE_INDICATOR
    } else {
        tokens::COLOR_TEXT_ON_CHROME_SECONDARY
    };
    let touch = if min_touch_px > 0.0 {
        format!("min-height: {min_touch_px}px;")
    } else {
        String::new()
    };
    let link = format!(
        "background: transparent; border: none; cursor: pointer; padding: 0; \
         {touch} font-size: {fs}px; color: {accent_fg}; \
         text-decoration: underline; text-align: left;",
        fs = tokens::FONT_SIZE_LABEL,
        accent_fg = tokens::COLOR_TAB_ACTIVE_INDICATOR,
    );

    rsx! {
        div {
            style: format!(
                "display: flex; flex-direction: row; align-items: center; \
                 flex-wrap: wrap; gap: {gap}px; font-size: {fs}px; color: {fg};",
                gap = tokens::SPACE_1,
                fs = tokens::FONT_SIZE_LABEL,
            ),

            span {
                style: format!("color: {fg};"),
                {kind.marker()}
            }
            span { {text} }

            // Jump to the ancestor that owns the value (inherited rows).
            if let (Some(label), Some(handler)) = (jump_label, on_jump) {
                span { style: format!("color: {};", tokens::COLOR_BORDER_CHROME), "\u{00B7}" }
                button {
                    style: link.clone(),
                    onclick: move |evt| {
                        evt.stop_propagation();
                        handler.call(());
                    },
                    {label}
                }
            }

            // Reset the local override (local rows only).
            if let (Some(label), Some(handler)) = (reset_label, on_reset) {
                span { style: format!("color: {};", tokens::COLOR_BORDER_CHROME), "\u{00B7}" }
                button {
                    style: link,
                    onclick: move |evt| {
                        evt.stop_propagation();
                        handler.call(());
                    },
                    {label}
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Exactly one kind is resettable — asserting only the Local case would
    /// pass for a predicate that is true everywhere.
    #[test]
    fn only_local_is_local() {
        assert!(AtProvenanceKind::Local.is_local());
        for kind in [
            AtProvenanceKind::Inherited,
            AtProvenanceKind::Default,
            AtProvenanceKind::Engine,
        ] {
            assert!(!kind.is_local(), "{kind:?} must not offer a reset");
        }
    }

    /// The marker carries the local/inherited distinction without colour, so it
    /// survives at 1-bit on E-Ink.
    #[test]
    fn the_marker_distinguishes_local_from_every_inherited_level() {
        let local = AtProvenanceKind::Local.marker();
        for kind in [
            AtProvenanceKind::Inherited,
            AtProvenanceKind::Default,
            AtProvenanceKind::Engine,
        ] {
            assert_ne!(local, kind.marker());
        }
    }
}
