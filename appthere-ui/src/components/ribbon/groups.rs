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
use crate::components::overlay::use_backdrop;
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

    // Outside-click dismissal: while the menu is open, the app's window-level
    // backdrop host (when wired — `use_provide_backdrop` + `AtBackdropHost`)
    // shows a viewport-spanning click-catcher that closes it. Degrades to
    // toggle-only dismissal in an app without the host.
    let backdrop = use_backdrop();
    let mut set_menu_open = move |open: bool| {
        menu_open.set(open);
        if let Some(b) = backdrop {
            if open {
                b.show(Callback::new(move |()| menu_open.set(false)));
            } else {
                b.hide();
            }
        }
    };
    // Never leave a stale backdrop if this strip unmounts with the menu open
    // (e.g. a ribbon tab switch).
    use_drop(move || {
        if let Some(b) = backdrop {
            if *menu_open.peek() {
                b.hide();
            }
        }
    });

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
        set_menu_open(false);
    }

    // The last in-strip group suppresses its trailing divider when nothing
    // follows it (no "More" button) — no divider dangles at the strip edge.
    let last_rendered = cascade
        .states
        .iter()
        .rposition(|s| *s != GroupCollapse::Overflow);

    rsx! {
        // In-strip groups, each at its resolved collapse state.
        for (idx, (spec, state)) in groups.iter().zip(cascade.states.iter()).enumerate() {
            if *state != GroupCollapse::Overflow {
                AtRibbonGroup {
                    key: "{spec.aria_label}",
                    label: spec.label.clone(),
                    aria_label: spec.aria_label.clone(),
                    collapse: *state,
                    show_divider: cascade.overflow || last_rendered != Some(idx),
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
                        set_menu_open(!open);
                    },
                    AtIcon { path_d: LUCIDE_MORE_HORIZONTAL.to_string() }
                }

                if menu_open() {
                    // The menu opens upward (the ribbon sits at the window bottom),
                    // anchored to the More button. `position: absolute` (block-level)
                    // is confirmed working in the current Blitz stack (see CLAUDE.md).
                    //
                    // Dismissal: outside-click via the window-level backdrop host
                    // (raised in `set_menu_open`; the menu's z-index 41 sits above
                    // `BACKDROP_Z_INDEX` 40 so its own controls stay clickable),
                    // plus the More-button toggle and the auto-close on a widen
                    // that removes the overflow.
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
                        // Overflowed groups render in Full form inside the
                        // menu — stacked vertically, so no right divider.
                        for spec in overflowed.iter() {
                            AtRibbonGroup {
                                key: "{spec.aria_label}",
                                label: spec.label.clone(),
                                aria_label: spec.aria_label.clone(),
                                collapse: GroupCollapse::Full,
                                show_divider: false,
                                {spec.content.clone()}
                            }
                        }
                    }
                }
            }
        }
    }
}
