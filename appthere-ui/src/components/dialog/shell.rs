// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 AppThere Loki contributors

//! [`AtDialogShell`] — the frame every tabbed Loki dialog shares: a modal
//! backdrop, a header (title · subtitle · close), a tab-strip slot, a scrolling
//! body, and a footer.
//!
//! # Mounting contract
//!
//! Rendered as a `position: absolute` full-area backdrop, so the **mounting
//! parent (or an ancestor) must be `position: relative` and span the area to
//! dim** — the same confirmed-working Blitz pattern as [`super::super::confirm_dialog`]
//! (`position: fixed` collapses to `absolute` in `stylo_taffy` and must not be
//! used). Mount it conditionally *at a component boundary* so it owns a hook
//! scope and can read the breakpoint itself (ADR-0013):
//!
//! ```rust,ignore
//! {open().then(|| rsx! { ParagraphStyleDialog { .. } })}
//! ```
//!
//! # Touch targets
//!
//! The close button is [`tokens::TOUCH_MIN`] square at every size class (WCAG
//! 2.5.8) — it is icon-only, so it never had a text-relative size to fall back
//! on. Footer actions reach the same minimum at Compact via
//! [`super::DialogPosture::touch_min_css`].

use dioxus::prelude::*;

use super::posture::{DialogPosture, DialogWidth};
use crate::responsive::use_breakpoint;
use crate::tokens;

/// The shared modal frame. `tabs`, `body` and `footer` are slots so each dialog
/// supplies its own content while inheriting the chrome and responsive posture.
#[component]
pub fn AtDialogShell(
    /// Dialog title — `Paragraph style — Body indent`.
    title: String,
    /// Secondary line under the title: a breadcrumb, a selection summary, or a
    /// size estimate. `None` renders a single-line header.
    #[props(default)]
    subtitle: Option<Element>,
    /// Accessible label for the close button.
    close_aria_label: String,
    /// Whether the card is a wide (preview-rail) or narrow dialog.
    #[props(default = DialogWidth::Wide)]
    width: DialogWidth,
    /// The tab strip, if this dialog has one.
    #[props(default)]
    tabs: Option<Element>,
    /// The active tab's body.
    body: Element,
    /// The footer row — secondary action, then Cancel / primary.
    footer: Element,
    /// Fired by the close button and by a backdrop click.
    on_close: EventHandler<()>,
) -> Element {
    let posture = DialogPosture::for_breakpoint(use_breakpoint());
    let title_size = if posture.full_screen {
        tokens::FONT_SIZE_SUBHEADING
    } else {
        tokens::FONT_SIZE_HEADING
    };
    // The card is labelled by its title and also renders it; take the copy the
    // `aria-label` needs before the heading consumes the prop.
    let aria_title = title.clone();

    rsx! {
        // Backdrop — dims and click-blocks the surface behind the dialog.
        // A Compact sheet fills the frame, so the dim is invisible there; it
        // still blocks, which is the part that matters.
        div {
            style: format!(
                "position: absolute; top: 0; left: 0; width: 100%; height: 100%; \
                 z-index: 2000; background: rgba(0, 0, 0, 0.45); display: flex; \
                 align-items: center; justify-content: center; padding: {pad}px; \
                 box-sizing: border-box;",
                pad = if posture.full_screen { 0.0 } else { tokens::SPACE_5 },
            ),
            role: "presentation",
            onclick: move |_| on_close.call(()),

            // The card. Clicks inside must not reach the backdrop's close.
            div {
                role: "dialog",
                aria_modal: "true",
                aria_label: aria_title,
                style: format!(
                    "{w} max-height: 100%; box-sizing: border-box; display: flex; \
                     flex-direction: column; overflow: hidden; background: {bg}; \
                     border: 1px solid {border}; border-radius: {r}px;",
                    w = posture.card_width_css(width),
                    bg = tokens::COLOR_SURFACE_1,
                    border = tokens::COLOR_BORDER_CHROME,
                    r = posture.card_radius_px(),
                ),
                onclick: move |evt| evt.stop_propagation(),

                // ── Header ────────────────────────────────────────────────────
                div {
                    style: format!(
                        "flex-shrink: 0; display: flex; flex-direction: row; \
                         align-items: flex-start; justify-content: space-between; \
                         gap: {gap}px; padding: {py}px {px}px; \
                         border-bottom: 1px solid {border};",
                        gap = tokens::SPACE_4,
                        py = tokens::SPACE_4,
                        px = tokens::SPACE_5,
                        border = tokens::COLOR_BORDER_CHROME,
                    ),
                    div {
                        style: format!(
                            "display: flex; flex-direction: column; gap: {gap}px; min-width: 0;",
                            gap = tokens::SPACE_1,
                        ),
                        div {
                            style: format!(
                                "font-size: {fs}px; font-weight: {fw}; color: {fg};",
                                fs = title_size,
                                fw = tokens::FONT_WEIGHT_SEMIBOLD,
                                fg = tokens::COLOR_TEXT_ON_CHROME,
                            ),
                            {title}
                        }
                        if let Some(sub) = subtitle {
                            {sub}
                        }
                    }
                    button {
                        aria_label: close_aria_label,
                        style: format!(
                            "width: {t}px; height: {t}px; flex-shrink: 0; \
                             display: flex; align-items: center; justify-content: center; \
                             background: transparent; border: none; cursor: pointer; \
                             border-radius: {r}px; font-size: {fs}px; color: {fg};",
                            t = tokens::TOUCH_MIN,
                            r = tokens::RADIUS_MD,
                            fs = tokens::FONT_SIZE_MD,
                            fg = tokens::COLOR_TEXT_ON_CHROME_SECONDARY,
                        ),
                        onclick: move |evt| {
                            evt.stop_propagation();
                            on_close.call(());
                        },
                        "\u{2715}"
                    }
                }

                // ── Tab strip ─────────────────────────────────────────────────
                if let Some(strip) = tabs {
                    {strip}
                }

                // ── Body ──────────────────────────────────────────────────────
                // COMPAT(dioxus-native): overflow-y: auto is confirmed working.
                div {
                    style: "flex: 1; min-height: 0; display: flex; overflow-y: auto;",
                    {body}
                }

                // ── Footer ────────────────────────────────────────────────────
                div {
                    style: format!(
                        "flex-shrink: 0; display: flex; flex-direction: {dir}; \
                         align-items: {align}; justify-content: space-between; \
                         gap: {gap}px; padding: {py}px {px}px; \
                         border-top: 1px solid {border};",
                        dir = posture.footer_direction(),
                        align = if posture.stack_footer { "stretch" } else { "center" },
                        gap = tokens::SPACE_2,
                        py = tokens::SPACE_3,
                        px = tokens::SPACE_5,
                        border = tokens::COLOR_BORDER_CHROME,
                    ),
                    {footer}
                }
            }
        }
    }
}
