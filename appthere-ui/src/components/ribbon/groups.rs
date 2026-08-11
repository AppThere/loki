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
use std::collections::HashMap;
use std::rc::Rc;

use super::overflow_menu::{overflow_menu_request, OVERFLOW_POPOVER_ID, PARTIAL_POPOVER_ID};
use crate::components::icons::{AtIcon, LUCIDE_MORE_HORIZONTAL};
use crate::components::popover::{use_popover_anchor, Rect};
use crate::responsive::{use_ribbon_cascade, GroupCollapse, GroupMetrics};
use crate::{use_safe_area, use_window_size};

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
    /// §11 partial-overflow opt-in. `Some` requires
    /// [`GroupMetrics::partial_px`] to be set (build the metrics via
    /// [`estimate_partial_metrics`](crate::estimate_partial_metrics)) —
    /// without it the cascade never assigns the Partial state and the spec is
    /// inert. `None` keeps the whole-group behaviour.
    pub partial: Option<RibbonPartialSpec>,
}

/// What a group shows in the [`GroupCollapse::Partial`] state (§11): its
/// retained controls stay in-strip, everything else lives behind a per-group
/// submenu chip that opens the group **in full** as a hosted popover.
#[derive(Clone, PartialEq)]
pub struct RibbonPartialSpec {
    /// The retained (always-visible) controls — typically the group's two or
    /// three most-used buttons.
    pub retained: Element,
    /// Accessible name for the per-group submenu chip.
    pub more_aria_label: String,
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

    // Both the menu and its dismiss backdrop are the popover host's (I-28). The
    // manual `use_backdrop` this replaced raised a root-sibling click-catcher
    // over a menu rendered inside `Router`, and Blitz builds no stacking
    // contexts — so the backdrop was hit first and the menu's own controls were
    // dead while it was open. One owner, one lifetime.
    let popover = use_popover_anchor(OVERFLOW_POPOVER_ID);
    let window = use_window_size();
    let insets = use_safe_area();
    let mut anchor = use_signal(|| Option::<MountedEvent>::None);
    let mut anchor_rect = use_signal(|| Option::<Rect>::None);

    // §11 Partial submenu: at most one open at a time, keyed by group index.
    // Chip rects are captured per group on mount (chips mount and unmount as
    // the cascade moves groups in and out of Partial).
    let partial_popover = use_popover_anchor(PARTIAL_POPOVER_ID);
    let mut partial_open = use_signal(|| Option::<usize>::None);
    let partial_rects = use_signal(HashMap::<usize, Rect>::new);

    // Partition into in-strip groups (with their state) and overflowed groups.
    let overflowed: Vec<RibbonGroupSpec> = groups
        .iter()
        .zip(&cascade.states)
        .filter(|(_, s)| **s == GroupCollapse::Overflow)
        .map(|(g, _)| g.clone())
        .collect();

    // A widen (or content change) that removes the overflow must not leave a
    // stale-open menu — its "More" button is gone, so it could never be toggled
    // shut. Reconcile in-render; it converges in one frame.
    //
    // `set`, not the old `set_menu_open`: the backdrop it also had to hide is
    // the host's now, and the effect below dismisses on the same signal.
    if !cascade.overflow && *menu_open.peek() {
        menu_open.set(false);
    }
    // Same reconciliation for the partial submenu: if its group left the
    // Partial state (widened to Condensed/Full or overflowed entirely), the
    // chip is gone and the menu must not outlive it.
    let stale_partial = partial_open
        .peek()
        .is_some_and(|idx| cascade.states.get(idx) != Some(&GroupCollapse::Partial));
    if stale_partial {
        partial_open.set(None);
    }

    // The open/dismiss write, in an effect rather than in the render — the shape
    // every popover consumer uses, so opening is a state change and the host
    // owns the mounting.
    //
    // **The dismiss branch is the ordinary path here**, unlike the spelling
    // menu's. That component is mounted behind `if menu.is_some()`, so closing
    // unmounts it and `use_popover_anchor`'s drop does the work. This strip
    // stays mounted with the menu shut, so nothing would dismiss it but this.
    {
        let overflowed = overflowed.clone();
        use_effect(move || {
            let Some(popover) = popover else { return };
            if !menu_open() {
                popover.dismiss();
                return;
            }
            let Some(rect) = *anchor_rect.read() else {
                // No measured trigger yet: `onmounted` and `get_client_rect`
                // both land after the first render. Opening without a rect
                // would place the menu at the origin, so wait — the effect
                // re-runs when the rect arrives.
                return;
            };
            let close: Rc<dyn Fn()> = Rc::new(move || {
                let mut menu_open = menu_open;
                menu_open.set(false);
            });
            popover.open(
                overflow_menu_request(
                    OVERFLOW_POPOVER_ID,
                    overflowed.clone(),
                    rect,
                    anchor.peek().as_ref().map(|e: &MountedEvent| e.data()),
                    close,
                ),
                window,
                insets,
            );
        });
    }

    // The partial submenu's open/dismiss effect — the same shape as the
    // overflow menu's above, keyed by the open group index.
    {
        let submenu_groups: Vec<RibbonGroupSpec> = groups.clone();
        use_effect(move || {
            let Some(popover) = partial_popover else {
                return;
            };
            let Some(idx) = partial_open() else {
                popover.dismiss();
                return;
            };
            let Some(rect) = partial_rects.read().get(&idx).copied() else {
                return; // chip not measured yet; re-runs when the rect lands
            };
            let Some(spec) = submenu_groups.get(idx) else {
                return;
            };
            let close: Rc<dyn Fn()> = Rc::new(move || {
                let mut partial_open = partial_open;
                partial_open.set(None);
            });
            popover.open(
                overflow_menu_request(PARTIAL_POPOVER_ID, vec![spec.clone()], rect, None, close),
                window,
                insets,
            );
        });
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
            if *state == GroupCollapse::Partial && spec.partial.is_some() {
                // §11: retained controls + the per-group submenu chip. The
                // spec carries `partial` (checked above); the fallback arm
                // below covers the impossible None to keep this total.
                AtRibbonGroup {
                    key: "{spec.aria_label}",
                    label: spec.label.clone(),
                    aria_label: spec.aria_label.clone(),
                    collapse: *state,
                    show_divider: cascade.overflow || last_rendered != Some(idx),
                    if let Some(partial) = spec.partial.as_ref() {
                        {partial.retained.clone()}
                        AtRibbonIconButton {
                            aria_label: partial.more_aria_label.clone(),
                            is_active: partial_open() == Some(idx),
                            is_disabled: false,
                            on_mounted: move |e: MountedEvent| {
                                let mut rects = partial_rects;
                                spawn(async move {
                                    if let Ok(r) = e.get_client_rect().await {
                                        rects.write().insert(idx, Rect {
                                            x: r.origin.x as f32,
                                            y: r.origin.y as f32,
                                            width: r.size.width as f32,
                                            height: r.size.height as f32,
                                        });
                                    }
                                });
                            },
                            on_click: move |_| {
                                let next = if partial_open() == Some(idx) { None } else { Some(idx) };
                                partial_open.set(next);
                            },
                            AtIcon { path_d: LUCIDE_MORE_HORIZONTAL.to_string() }
                        }
                    }
                }
            } else if *state != GroupCollapse::Overflow {
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

        // Overflow ("More") button. The menu itself is the host's — see the
        // effect above and `overflow_menu`.
        if cascade.overflow {
            AtRibbonIconButton {
                aria_label: overflow_aria_label,
                is_active: menu_open(),
                is_disabled: false,
                on_mounted: move |e: MountedEvent| {
                    let el = e.clone();
                    anchor.set(Some(e));
                    // The rect is read once, asynchronously: the placement needs
                    // where the trigger *is*, and `MountedData` only answers
                    // that through a future.
                    spawn(async move {
                        if let Ok(r) = el.get_client_rect().await {
                            anchor_rect.set(Some(Rect {
                                x: r.origin.x as f32,
                                y: r.origin.y as f32,
                                width: r.size.width as f32,
                                height: r.size.height as f32,
                            }));
                        }
                    });
                },
                on_click: move |_| {
                    let open = menu_open();
                    menu_open.set(!open);
                },
                AtIcon { path_d: LUCIDE_MORE_HORIZONTAL.to_string() }
            }
        }
    }
}
