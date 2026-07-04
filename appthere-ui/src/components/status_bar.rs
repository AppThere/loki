// SPDX-License-Identifier: Apache-2.0

//! `AtStatusBar` — bottom status bar shell component.
//!
//! Migrated and generalised from `loki-text/src/components/toolbar.rs::BottomToolbar`.
//! All previously hardcoded strings have been replaced with props.

use dioxus::prelude::*;

use crate::responsive::use_breakpoint;
use crate::tokens::colors::{
    COLOR_BORDER_CHROME, COLOR_CONTEXTUAL_TAB, COLOR_SURFACE_3, COLOR_SURFACE_CHROME,
    COLOR_TEXT_ACCENT, COLOR_TEXT_ON_CHROME_SECONDARY,
};
use crate::tokens::layout::STATUS_BAR_HEIGHT;
use crate::tokens::spacing::{RADIUS_SM, SPACE_1, SPACE_2, SPACE_4};
use crate::tokens::typography::{FONT_FAMILY_UI, FONT_SIZE_XS, FONT_WEIGHT_MEDIUM};

// ── AtStatusBar ───────────────────────────────────────────────────────────────

/// Bottom application status bar.
///
/// Displays document statistics on the left and navigation controls (zoom,
/// language, collaborator count) on the right.
///
/// **Minimum interactive size: 44×44 logical pixels (WCAG 2.5.8).**
///
/// The zoom badge is smaller than `TOUCH_MIN` in its visual footprint.
/// TODO(a11y): Expand the invisible touch target to `TOUCH_MIN` using padding.
#[component]
pub fn AtStatusBar(props: AtStatusBarProps) -> Element {
    let mut zoom_hovered = use_signal(|| false);
    let zoom_bg = if zoom_hovered() {
        "#444444"
    } else {
        COLOR_SURFACE_3
    };
    let mut view_hovered = use_signal(|| false);
    let view_bg = if view_hovered() {
        "#444444"
    } else {
        COLOR_SURFACE_3
    };
    let mut notice_hovered = use_signal(|| false);
    let notice_bg = if notice_hovered() {
        "#444444"
    } else {
        COLOR_SURFACE_3
    };
    let show_view_toggle = !props.view_mode_label.is_empty();
    let show_notice = !props.notice_label.is_empty();
    // Compact (phone-width): drop the secondary stats (word count, language)
    // so the essential page / notice / view-mode / zoom items don't crowd.
    let compact = use_breakpoint().is_compact();

    rsx! {
        div {
            role: "status",
            "aria-live": "polite",
            style: format!(
                "height: {h}px; min-height: {h}px; background: {bg}; \
                 border-top: 1px solid {border}; \
                 display: flex; align-items: center; \
                 padding: 0 {pad}px; flex-shrink: 0; gap: {gap}px; \
                 font-family: {font};",
                h      = STATUS_BAR_HEIGHT,
                bg     = COLOR_SURFACE_CHROME,
                border = COLOR_BORDER_CHROME,
                pad    = SPACE_4,
                gap    = SPACE_4,
                font   = FONT_FAMILY_UI,
            ),

            // ── Left: document statistics ─────────────────────────────────────

            // Page label (e.g. "Page 1 of 4"). Hidden when empty (e.g. the
            // reflow view, which has no fixed pages).
            if !props.page_label.is_empty() {
                span {
                    style: format!(
                        "font-size: {size}px; color: {fg};",
                        size = FONT_SIZE_XS,
                        fg   = COLOR_TEXT_ON_CHROME_SECONDARY,
                    ),
                    "{props.page_label}"
                }
            }

            // Word count label (e.g. "1,847 words"). Hidden when empty or at
            // Compact width (secondary stat).
            if !props.word_count_label.is_empty() && !compact {
                span {
                    style: format!(
                        "font-size: {size}px; color: {fg};",
                        size = FONT_SIZE_XS,
                        fg   = COLOR_TEXT_ON_CHROME_SECONDARY,
                    ),
                    "{props.word_count_label}"
                }
            }

            // Optional status notice chip (e.g. recover a dismissed warning).
            // Border + background only — no box-shadow (Blitz constraint).
            if show_notice {
                button {
                    "aria-label": props.notice_aria_label.clone(),
                    style: format!(
                        "background: {bg}; border: 1px solid {border}; border-radius: {r}px; \
                         color: {fg}; font-size: {size}px; font-weight: {weight}; \
                         cursor: pointer; padding: {pv}px {ph}px; flex-shrink: 0;",
                        bg     = notice_bg,
                        border = COLOR_CONTEXTUAL_TAB,
                        r      = RADIUS_SM,
                        fg     = COLOR_TEXT_ON_CHROME_SECONDARY,
                        size   = FONT_SIZE_XS,
                        weight = FONT_WEIGHT_MEDIUM,
                        pv     = SPACE_1,
                        ph     = SPACE_2,
                    ),
                    onmouseenter: move |_| { notice_hovered.set(true); },
                    onmouseleave: move |_| { notice_hovered.set(false); },
                    onclick: move |_| { props.on_notice_click.call(()); },
                    "⚠ {props.notice_label}"
                }
            }

            // Flex spacer — pushes right-side content to the far right
            div { style: "flex: 1;" }

            // ── Right: language, zoom, collaborators ──────────────────────────

            // Language label (e.g. "English (US)"). Hidden at Compact width.
            if !compact {
                span {
                    style: format!(
                        "font-size: {size}px; color: {fg};",
                        size = FONT_SIZE_XS,
                        fg   = COLOR_TEXT_ON_CHROME_SECONDARY,
                    ),
                    "{props.language_label}"
                }
            }

            // View-mode toggle (paginated ⇆ reflowed). Hidden unless a label is
            // supplied, so apps that do not offer the toggle are unaffected.
            // **Minimum interactive size: 44×44 logical pixels (WCAG 2.5.8).**
            // TODO(a11y): expand the invisible touch target to TOUCH_MIN.
            if show_view_toggle {
                button {
                    "aria-label": props.view_mode_aria_label.clone(),
                    style: format!(
                        "background: {bg}; border: none; border-radius: {r}px; \
                         color: {fg}; font-size: {size}px; font-weight: {weight}; \
                         cursor: pointer; padding: {pv}px {ph}px; flex-shrink: 0;",
                        bg     = view_bg,
                        r      = RADIUS_SM,
                        fg     = COLOR_TEXT_ON_CHROME_SECONDARY,
                        size   = FONT_SIZE_XS,
                        weight = FONT_WEIGHT_MEDIUM,
                        pv     = SPACE_1,
                        ph     = SPACE_2,
                    ),
                    onmouseenter: move |_| { view_hovered.set(true); },
                    onmouseleave: move |_| { view_hovered.set(false); },
                    onclick: move |_| { props.on_view_mode_click.call(()); },
                    "{props.view_mode_label}"
                }
            }

            // Zoom badge (clickable button)
            // The zoom badge is smaller than TOUCH_MIN in its visual footprint.
            // TODO(a11y): Expand the invisible touch target to TOUCH_MIN using padding.
            button {
                "aria-label": props.zoom_aria_label,
                style: format!(
                    "background: {bg}; border: none; border-radius: {r}px; \
                     color: {fg}; font-size: {size}px; font-weight: {weight}; \
                     cursor: pointer; padding: {pv}px {ph}px; flex-shrink: 0;",
                    bg     = zoom_bg,
                    r      = RADIUS_SM,
                    fg     = COLOR_TEXT_ON_CHROME_SECONDARY,
                    size   = FONT_SIZE_XS,
                    weight = FONT_WEIGHT_MEDIUM,
                    pv     = SPACE_1,
                    ph     = SPACE_2,
                ),
                onmouseenter: move |_| { zoom_hovered.set(true); },
                onmouseleave: move |_| { zoom_hovered.set(false); },
                onclick: move |_| { props.on_zoom_click.call(()); },
                "{props.zoom_percent}%"
            }

            // Collaborator badge (hidden when count is 0)
            if props.collaborator_count > 0 {
                span {
                    style: format!(
                        "font-size: {size}px; color: {fg};",
                        size = FONT_SIZE_XS,
                        fg   = COLOR_TEXT_ACCENT,
                    ),
                    "{props.collaborator_label}"
                }
            }
        }
    }
}

// ── Props ─────────────────────────────────────────────────────────────────────

/// Props for [`AtStatusBar`].
#[derive(Props, Clone, PartialEq)]
pub struct AtStatusBarProps {
    /// Pre-formatted page label, e.g. `"Page 1 of 4"`.
    pub page_label: String,

    /// Pre-formatted word count label, e.g. `"1,847 words"`.
    pub word_count_label: String,

    /// Active language label, e.g. `"English (US)"`.
    pub language_label: String,

    /// Zoom percentage value, e.g. `100` (rendered as `"100%"`).
    pub zoom_percent: u32,

    /// Number of active remote collaborators. `0` = hide the collaborator badge.
    pub collaborator_count: u32,

    /// Pre-formatted collaborator label, e.g. `"2 connected"`.
    /// Only rendered when `collaborator_count > 0`.
    pub collaborator_label: String,

    /// Callback invoked when the zoom badge is clicked.
    pub on_zoom_click: EventHandler<()>,

    /// Aria label for the zoom button.
    pub zoom_aria_label: String,

    /// Label for the optional view-mode toggle (e.g. `"Paginated"`/`"Reflowed"`).
    /// Empty (the default) hides the toggle, so apps that do not offer it are
    /// unaffected.
    #[props(default)]
    pub view_mode_label: String,

    /// Aria label for the view-mode toggle button.
    #[props(default)]
    pub view_mode_aria_label: String,

    /// Callback invoked when the view-mode toggle is clicked. Defaults to a
    /// no-op when not provided.
    #[props(default)]
    pub on_view_mode_click: Callback<()>,

    /// Optional status-notice chip rendered on the left (e.g. the recovery
    /// affordance for a dismissed font-substitution warning). Empty (the
    /// default) hides it, so apps that do not use it are unaffected. Generic by
    /// design — not font-specific.
    ///
    /// **Min interactive size: 44×44 logical px (WCAG 2.5.8).** TODO(a11y):
    /// expand the invisible touch target to `TOUCH_MIN` (shared with the zoom /
    /// view-mode badges, which carry the same status-bar-height constraint).
    #[props(default)]
    pub notice_label: String,

    /// Aria label for the notice chip.
    #[props(default)]
    pub notice_aria_label: String,

    /// Callback invoked when the notice chip is clicked.
    #[props(default)]
    pub on_notice_click: Callback<()>,
}
