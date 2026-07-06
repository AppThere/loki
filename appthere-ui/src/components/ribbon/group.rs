// SPDX-License-Identifier: Apache-2.0

//! [`AtRibbonGroup`] — a labelled cluster of related ribbon buttons.
//!
//! Groups are separated from adjacent groups by a subtle right-side vertical
//! divider.  The label (if provided) appears below the button row.
//!
//! # Touch target
//!
//! `AtRibbonGroup` is a structural layout container, not itself interactive.
//! Individual buttons placed inside the group must each meet the 44 × 44 px
//! WCAG 2.5.8 minimum touch target.  The group's content row is
//! [`RIBBON_CONTENT_HEIGHT`](appthere_ui::tokens::RIBBON_CONTENT_HEIGHT) px
//! (60 px) tall, which comfortably accommodates standard ribbon button sizes.

use dioxus::prelude::*;

use crate::responsive::{group_layout, GroupCollapse};
use crate::tokens;
use crate::tokens::FONT_FAMILY_UI;

/// A labelled cluster of related ribbon buttons with a vertical divider.
///
/// The [`collapse`](AtRibbonGroupProps::collapse) prop drives the group through
/// the Spec 04 M3 §7 cascade:
///
/// - [`GroupCollapse::Full`] — labelled group, normal padding (the default).
/// - [`GroupCollapse::Condensed`] — the label drops and the buttons pack tighter
///   (narrower padding / gap) to reclaim strip width before any group overflows.
/// - [`GroupCollapse::Overflow`] — the group has moved into the overflow ("More")
///   menu, so it renders **nothing** in the strip (the menu hosts it instead).
///
/// # Minimum touch target
///
/// This component is a layout container; buttons inside must individually
/// satisfy the 44 × 44 px WCAG 2.5.8 minimum touch target. The condensed state
/// only tightens the *inter-control* spacing, never the buttons' own size, so
/// touch targets are preserved.
#[component]
pub fn AtRibbonGroup(
    /// Short label shown below the button row (e.g. "Clipboard").
    /// Pass `None` to omit the label. The label is also hidden in the
    /// [`GroupCollapse::Condensed`] state regardless of this value.
    label: Option<String>,
    /// ARIA group label for accessibility (`role="group"` `aria-label`).
    aria_label: String,
    /// Cascade display state (Spec 04 M3 §7). Defaults to [`GroupCollapse::Full`]
    /// so existing call sites keep their full labelled appearance.
    #[props(default = GroupCollapse::Full)]
    collapse: GroupCollapse,
    /// Buttons and controls inside this group.
    children: Element,
) -> Element {
    // The pure cascade helper decides render/pad/gap/label for this state.
    let lay = group_layout(collapse, label.is_some());
    // Overflowed groups live in the "More" menu, not the strip.
    if !lay.rendered {
        return rsx! {};
    }

    rsx! {
        div {
            role: "group",
            aria_label: aria_label,
            // Group label rendered as a flex column child below the buttons
            // rather than absolutely positioned at the group bottom. (Block-level
            // position: absolute is now confirmed working in Blitz — see
            // CLAUDE.md "Confirmed CSS properties" — so this in-flow layout is a
            // deliberate choice, no longer a Blitz limitation.)
            style: format!(
                "display: flex; flex-direction: column; align-items: center; \
                 height: 100%; padding: 0 {pad}px; \
                 border-right: 1px solid {border}; box-sizing: border-box;",
                // TODO(ribbon): Consider a variant prop to suppress the trailing
                // divider on the last group in a tab.
                pad    = lay.pad_px,
                border = tokens::COLOR_BORDER_CHROME,
            ),

            // Button row (fills available height minus optional label row)
            div {
                style: format!(
                    "display: flex; flex-direction: row; align-items: center; \
                     flex: 1; gap: {gap}px;",
                    gap = lay.gap_px,
                ),
                {children}
            }

            // Optional label row below buttons (dropped when condensed)
            if lay.show_label {
                div {
                    style: format!(
                        // TODO(font): verify Atkinson Hyperlegible Next is
                        // registered and loading correctly — group labels should
                        // not be in system-ui.
                        "font-family: {font}; font-size: {size}px; color: {fg}; \
                         text-align: center; padding-bottom: 2px; flex-shrink: 0;",
                        font = FONT_FAMILY_UI,
                        size = tokens::FONT_SIZE_XS,
                        fg   = tokens::COLOR_TEXT_ON_CHROME_SECONDARY,
                    ),
                    "{label.as_deref().unwrap_or_default()}"
                }
            }
        }
    }
}
