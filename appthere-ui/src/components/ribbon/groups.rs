// SPDX-License-Identifier: Apache-2.0

//! [`AtRibbonGroups`] — the collapse-aware container for a tab's groups.
//!
//! A tab supplies its groups as a `Vec<`[`RibbonGroupSpec`]`>` (each = its
//! collapse [`GroupMetrics`] plus the group's label / aria / button content).
//! This component runs the width-driven cascade
//! ([`use_ribbon_cascade`](crate::use_ribbon_cascade)) once for the whole strip
//! and renders each group in its resolved [`GroupCollapse`] state, moving
//! overflowed groups into a trailing "More" menu (Spec 04 M3 §6–§7).
//!
//! The framework owns the cascade so every app's tabs get the same behaviour;
//! the app only declares *what* the groups are, never *how* they collapse.

use dioxus::prelude::*;

use super::button::AtRibbonIconButton;
use super::group::AtRibbonGroup;
use crate::components::icons::{AtIcon, LUCIDE_MORE_HORIZONTAL};
use crate::responsive::{use_ribbon_cascade, GroupCollapse, GroupMetrics};
use crate::tokens;

/// One group's declaration: its collapse metrics, label, aria label, and the
/// button/control content (the group body, *without* the surrounding
/// [`AtRibbonGroup`] — the container wraps it at the resolved collapse state).
#[derive(Clone, PartialEq)]
pub struct RibbonGroupSpec {
    /// Collapse priority + full/condensed widths (see [`GroupMetrics`]; build via
    /// [`estimate_group_metrics`](crate::estimate_group_metrics) for icon groups).
    pub metrics: GroupMetrics,
    /// Group label shown below the buttons in the Full state (`None` = no label).
    pub label: Option<String>,
    /// ARIA group label.
    pub aria_label: String,
    /// The group's buttons/controls.
    pub content: Element,
}

/// Renders a tab's [`RibbonGroupSpec`]s through the width-driven collapse cascade.
///
/// # Touch target
///
/// Structural container. The "More" button and each group's buttons carry their
/// own 44 × 44 px targets (WCAG 2.5.8).
#[component]
pub fn AtRibbonGroups(
    /// The active tab's groups, left-to-right.
    groups: Vec<RibbonGroupSpec>,
    /// Accessible name for the overflow ("More") button — the translated
    /// "More controls" string from the caller.
    overflow_aria_label: String,
) -> Element {
    let metrics: Vec<GroupMetrics> = groups.iter().map(|g| g.metrics).collect();
    let cascade = use_ribbon_cascade(metrics);
    let mut menu_open = use_signal(|| false);

    // Partition into in-strip groups (with their state) and overflowed groups.
    let overflowed: Vec<&RibbonGroupSpec> = groups
        .iter()
        .zip(&cascade.states)
        .filter(|(_, s)| **s == GroupCollapse::Overflow)
        .map(|(g, _)| g)
        .collect();

    // A widen (or content change) that removes the overflow must not leave a
    // stale-open menu — its "More" button is gone, so it could never be toggled
    // shut. Reconcile in-render; it converges in one frame.
    if !cascade.overflow && *menu_open.peek() {
        menu_open.set(false);
    }

    rsx! {
        // In-strip groups, each at its resolved collapse state.
        for (spec, state) in groups.iter().zip(cascade.states.iter()) {
            if *state != GroupCollapse::Overflow {
                AtRibbonGroup {
                    key: "{spec.aria_label}",
                    label: spec.label.clone(),
                    aria_label: spec.aria_label.clone(),
                    collapse: *state,
                    {spec.content.clone()}
                }
            }
        }

        // Overflow ("More") button + upward dropdown when any group overflowed.
        if cascade.overflow {
            div {
                // Positioned wrapper so the dropdown anchors to the button.
                style: "position: relative; display: flex; align-items: center; \
                        height: 100%;",

                AtRibbonIconButton {
                    aria_label: overflow_aria_label,
                    is_active: menu_open(),
                    is_disabled: false,
                    on_click: move |_| {
                        let open = menu_open();
                        menu_open.set(!open);
                    },
                    AtIcon { path_d: LUCIDE_MORE_HORIZONTAL.to_string() }
                }

                if menu_open() {
                    // The menu opens upward (the ribbon sits at the window bottom),
                    // anchored to the More button. `position: absolute` (block-level)
                    // is confirmed working in the current Blitz stack (see CLAUDE.md).
                    //
                    // Dismissal: the More button toggles it shut, and a resize that
                    // removes the overflow auto-closes it (above). True
                    // outside-click-to-dismiss needs a full-viewport backdrop, which
                    // must be hosted at a positioned window-level ancestor (like the
                    // editor-root overlay the spell panel uses) — `position: fixed`
                    // collapses to `absolute` in stylo_taffy, so a backdrop rendered
                    // here would only cover this small wrapper. TODO(ribbon): host the
                    // overflow menu in a shared window-level overlay so a backdrop can
                    // span the viewport.
                    div {
                        style: format!(
                            "position: absolute; bottom: 100%; right: 0; z-index: 41; \
                             display: flex; flex-direction: column; gap: {gap}px; \
                             padding: {pad}px; background: {bg}; \
                             border: 1px solid {border}; border-radius: {radius}px;",
                            gap    = tokens::SPACE_2,
                            pad    = tokens::SPACE_2,
                            bg     = tokens::COLOR_SURFACE_2,
                            border = tokens::COLOR_BORDER_CHROME,
                            radius = tokens::RADIUS_MD,
                        ),
                        // Overflowed groups render in Full form inside the menu.
                        for spec in overflowed.iter() {
                            AtRibbonGroup {
                                key: "{spec.aria_label}",
                                label: spec.label.clone(),
                                aria_label: spec.aria_label.clone(),
                                collapse: GroupCollapse::Full,
                                {spec.content.clone()}
                            }
                        }
                    }
                }
            }
        }
    }
}
