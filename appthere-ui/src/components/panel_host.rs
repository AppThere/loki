// SPDX-License-Identifier: Apache-2.0

//! `AtPanelHost` — the sanctioned host for a conditionally-mounted panel
//! (Spec 05 §10 / ADR-0013).
//!
//! # The pattern
//!
//! A panel shown behind a condition must **not** be a plain function called
//! inside `if cond { panel(..) }` with an early `return rsx!{}`: that body has no
//! component identity, so it cannot host a hook — including
//! [`use_breakpoint`](crate::responsive::use_breakpoint) — and threading a
//! `compact` flag down from the parent grows the parent (the `editor_inner.rs`
//! ceiling pressure that deferred Spec 03's R-13g).
//!
//! Instead, a panel is a `#[component]` mounted **at the boundary**:
//! ```rust,ignore
//! {open().then(|| rsx! {
//!     AtPanelHost { title: fl!("styles-title"), on_close: move |_| open.set(false),
//!         StyleInspector { /* … */ }
//!     }
//! })}
//! ```
//! The component owns its hook scope (Dioxus manages it across mount/unmount), so
//! it reads the resilient `use_breakpoint()` directly and adapts posture with no
//! prop threading and no growth of the mounting parent.
//!
//! `AtPanelHost` is that boundary: it renders a titled, closable container and
//! picks its posture from the size class ([`PanelPosture::for_breakpoint`]) — a
//! full-width touch-first sheet at Compact, a bounded side panel at
//! Medium/Expanded. Elevation is token border/background (no `box-shadow`); it
//! lives in flow (no `position: fixed`) per the Blitz constraints. Spec 05's
//! family panels mount their inspector inside it, so each is born able to read
//! the breakpoint.

use dioxus::prelude::*;

use crate::responsive::{use_breakpoint, Breakpoint};
use crate::tokens::colors::{COLOR_BORDER_CHROME, COLOR_SURFACE_1, COLOR_TEXT_ON_CHROME};
use crate::tokens::layout::PANEL_SIDE_WIDTH_PX;
use crate::tokens::spacing::{RADIUS_MD, SPACE_2, SPACE_3, SPACE_4, TOUCH_MIN};
use crate::tokens::typography::{FONT_FAMILY_UI, FONT_SIZE_MD, FONT_WEIGHT_SEMIBOLD};

/// Responsive posture for a hosted panel, derived **purely** from the size class
/// so it is testable without a real window (Spec 03 D1).
///
/// At Compact the panel is a full-width, touch-first sheet (no room beside the
/// document); at Medium/Expanded it is a bounded side panel.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct PanelPosture {
    /// Fill the available width (a Compact sheet) rather than sit at [`width_px`].
    ///
    /// [`width_px`]: PanelPosture::width_px
    pub full_width: bool,
    /// Bounded panel width (logical px) when not [`full_width`]. Ignored when
    /// `full_width` is `true`.
    ///
    /// [`full_width`]: PanelPosture::full_width
    pub width_px: f32,
}

impl PanelPosture {
    /// The posture for a size class.
    #[must_use]
    pub fn for_breakpoint(bp: Breakpoint) -> Self {
        if bp.is_compact() {
            Self {
                full_width: true,
                width_px: PANEL_SIDE_WIDTH_PX,
            }
        } else {
            Self {
                full_width: false,
                width_px: PANEL_SIDE_WIDTH_PX,
            }
        }
    }

    /// The CSS `width` value this posture resolves to.
    #[must_use]
    pub fn css_width(self) -> String {
        if self.full_width {
            "100%".to_string()
        } else {
            format!("{}px", self.width_px)
        }
    }
}

/// A responsive host for a conditionally-mounted panel — the sanctioned
/// conditional-panel pattern (see the module docs and ADR-0013).
///
/// Mount it at a boundary (`{open().then(|| rsx! { AtPanelHost { .. } })}`); it
/// reads [`use_breakpoint`](crate::responsive::use_breakpoint) itself and adapts
/// posture via [`PanelPosture`] without any `compact` flag from the parent.
///
/// # Touch target
///
/// The close control is **44×44 logical px minimum** (WCAG 2.5.8, `TOUCH_MIN`).
#[component]
pub fn AtPanelHost(props: AtPanelHostProps) -> Element {
    let posture = PanelPosture::for_breakpoint(use_breakpoint());

    rsx! {
        section {
            role: "dialog",
            "aria-label": props.title.clone(),
            style: format!(
                "display: flex; flex-direction: column; box-sizing: border-box; \
                 width: {width}; background: {bg}; border: 1px solid {border}; \
                 border-radius: {radius}px; font-family: {font};",
                width  = posture.css_width(),
                bg     = COLOR_SURFACE_1,
                border = COLOR_BORDER_CHROME,
                radius = RADIUS_MD,
                font   = FONT_FAMILY_UI,
            ),

            // ── Header: title + close (TOUCH_MIN target) ──────────────────────
            div {
                style: format!(
                    "display: flex; align-items: center; gap: {gap}px; \
                     padding: {pv}px {ph}px; border-bottom: 1px solid {border};",
                    gap    = SPACE_2,
                    pv     = SPACE_2,
                    ph     = SPACE_4,
                    border = COLOR_BORDER_CHROME,
                ),
                span {
                    style: format!(
                        "flex: 1; color: {fg}; font-size: {size}px; font-weight: {weight};",
                        fg     = COLOR_TEXT_ON_CHROME,
                        size   = FONT_SIZE_MD,
                        weight = FONT_WEIGHT_SEMIBOLD,
                    ),
                    "{props.title}"
                }
                button {
                    "aria-label": props.close_aria_label.clone(),
                    style: format!(
                        "min-width: {t}px; min-height: {t}px; background: transparent; \
                         border: none; border-radius: {radius}px; color: {fg}; \
                         font-size: {size}px; cursor: pointer;",
                        t      = TOUCH_MIN,
                        radius = RADIUS_MD,
                        fg     = COLOR_TEXT_ON_CHROME,
                        size   = FONT_SIZE_MD,
                    ),
                    onclick: move |_| props.on_close.call(()),
                    "✕"
                }
            }

            // ── Body ──────────────────────────────────────────────────────────
            div {
                style: format!("padding: {p}px; overflow-y: auto;", p = SPACE_3),
                {props.children}
            }
        }
    }
}

/// Props for [`AtPanelHost`].
#[derive(Props, Clone, PartialEq)]
pub struct AtPanelHostProps {
    /// Localized panel title shown in the header.
    pub title: String,
    /// Aria label for the close control (localized).
    pub close_aria_label: String,
    /// Invoked when the user activates the close control.
    pub on_close: EventHandler<()>,
    /// Panel body.
    pub children: Element,
}

#[cfg(test)]
#[path = "panel_host_tests.rs"]
mod tests;
