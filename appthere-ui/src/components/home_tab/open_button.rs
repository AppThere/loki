// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 AppThere Loki contributors

//! The Recent section's Open action, and its tooltip (Spec 08 T4.3, I-02).
//!
//! # What moved, and why the button changed shape
//!
//! The action was a full-width **"Open file…" button below the list** — so on a
//! long list it sat below the fold of a container that scrolls, and the primary
//! way into the application was reachable only by scrolling past ten documents
//! to find it. It is now an icon button at the **top of the section**, beside the
//! heading, where it is visible whatever the list is doing.
//!
//! # The tooltip has its own `Role`, and it does not get a backdrop
//!
//! **Corrected r78.** This said a tooltip "is not a `Role`" — that giving it one
//! would put an arm in `route_key` that can never run. Wiring the first
//! dispatcher showed the opposite: every overlay the host renders reaches
//! `route_key`, so a tooltip filed under `Panel` had Tab *consumed* and routed to
//! an `on_key` it does not supply, and `autofocus` pulled focus out of whatever
//! the user was typing in. Both are live defects of the missing variant, and
//! [`crate::components::popover::Role::Tooltip`] is the fix.
//!
//! It shares the *host* — because the Recent section is `overflow-y: auto` and an
//! out-of-flow child is clipped by it, exactly as T4.2's menu was.
//!
//! The second is [`OverlayKind::PointerDriven`]: the host's backdrop is a
//! window-sized click-catcher, correct for a menu and catastrophic for a tooltip,
//! which would capture every click in the application for as long as a pointer
//! rested on this icon.
//!
//! # An unreachable tooltip is worse than a redundant label
//!
//! With a coarse pointer there is no hover, so a tooltip is not a weaker
//! affordance — it is **no affordance**, and an icon button with no visible name
//! is then a control the user has to guess. So the presentation branches on
//! [`PointerPrecision::has_hover`], and `Unknown` takes the *label* path: until
//! we know, assume the affordance needs to be visible. That is the direction the
//! predicate's own docs choose, and it means a touch-only session is served
//! correctly from the first frame rather than after the first interaction.
//!
//! # Touch target
//!
//! The button is `TOUCH_MIN` × `TOUCH_MIN` (44 × 44, WCAG 2.5.8) in the icon
//! form, and at least that tall in the labelled form. `aria-label` carries the
//! action's name in **both** forms — the tooltip is a hover affordance, never the
//! accessible name, which is what "an independent accessible label" means in
//! I-02.

use std::rc::Rc;

use dioxus::prelude::*;

use crate::components::popover::{
    use_popover_anchor, OverlayKind, PopoverId, PopoverRequest, Rect, Role,
};

#[path = "open_button_place.rs"]
mod place_impl;
use crate::device_profile::PointerPrecision;
use crate::tokens::colors::{
    COLOR_ACCENT_PRIMARY, COLOR_ACCENT_PRIMARY_HOVER, COLOR_SURFACE_CHROME, COLOR_TEXT_ON_CHROME,
};
use crate::tokens::spacing::{RADIUS_SM, SPACE_2, SPACE_3, TOUCH_MIN};
use crate::tokens::typography::{FONT_SIZE_BODY, FONT_SIZE_LABEL};
use crate::{use_device_profile, use_safe_area, use_window_size};
pub(super) use place_impl::{shows_visible_label, tooltip_placement};

/// Identifies the Open tooltip to the popover singleton rule.
const TOOLTIP_POPOVER_ID: PopoverId = PopoverId(0x0_9E70);

/// Props for [`AtOpenAction`].
#[derive(Props, Clone, PartialEq)]
pub(super) struct AtOpenActionProps {
    /// The action's name — the accessible label in both forms, and the visible
    /// text in the labelled one.
    pub label: String,
    pub on_click: EventHandler<()>,
}

/// The Recent section's Open action.
///
/// # Touch target
///
/// 44 × 44 logical pixels minimum (WCAG 2.5.8) in both forms.
#[component]
pub(super) fn AtOpenAction(props: AtOpenActionProps) -> Element {
    let profile = use_device_profile();
    let popover = use_popover_anchor(TOOLTIP_POPOVER_ID);
    let window = use_window_size();
    let insets = use_safe_area();

    let mut hovered = use_signal(|| false);
    let mut trigger = use_signal(|| Option::<MountedEvent>::None);
    let mut tooltip_anchor = use_signal(|| Option::<Rect>::None);

    // Reads the live profile, so this switches **during** a session: plugging in
    // a mouse on an Android desktop device moves the presentation without a
    // relaunch, which is what "switching live" in I-02 asks for and what a
    // `cfg!(target_os)` answer could never do (L08-011).
    let labelled = shows_visible_label(profile.pointer);
    let label = props.label.clone();
    let tooltip_label = label.clone();

    // The tooltip request, pushed from an effect rather than a render.
    //
    // Gated on `labelled` as well as on hover: in the labelled form the name is
    // already on screen, so a tooltip would repeat it — and on a coarse pointer
    // `hovered` cannot become true anyway, which makes the second condition a
    // statement of intent rather than a live branch.
    use_effect(move || {
        let Some(anchor) = popover else {
            return;
        };
        let showing = hovered() && !labelled;
        let Some(rect) = *tooltip_anchor.read() else {
            return;
        };
        if !showing {
            anchor.dismiss();
            return;
        }
        let label = tooltip_label.clone();
        anchor.open(
            PopoverRequest {
                id: TOOLTIP_POPOVER_ID,
                placement: tooltip_placement(rect),
                // Nothing dismisses a tooltip by clicking; the pointer leaving
                // does, through `hovered`. Kept as a no-op rather than made
                // optional because the host always has an owner to call, and an
                // `Option` here would be a second way to say `PointerDriven`.
                on_dismiss: Rc::new(|| {}),
                on_outside_move: None,
                kind: OverlayKind::PointerDriven,
                // **Its own role, since r78.** It was `Panel` while `route_key`
                // had no caller; the first dispatcher showed that a `Panel`
                // consumes Tab and hands it to an `on_key` this does not
                // supply — a tooltip that swallows Tab — and that `autofocus`
                // would steal focus from whatever the pointer's owner was
                // typing in. See `Role::Tooltip`.
                role: Role::Tooltip,
                on_key: None,
                // **No anchor, on purpose.** A tooltip never took focus, so
                // there is nothing to give back: every one of its dismissals is
                // the pointer leaving, and moving focus to the button the
                // pointer just left would steal it from wherever the user
                // actually is. `None` here means `dismiss_sequence` takes the
                // fallback branch, which for `FocusTarget::Unchanged` is no
                // focus step at all.
                anchor: None,
                content: Rc::new(move || tooltip_content(label.clone())),
            },
            window,
            insets,
        );
    });

    let button_style = if labelled {
        format!(
            "background: {bg}; color: {fg}; border: none; border-radius: {r}px; \
             min-height: {touch}px; padding: 0 {ph}px; cursor: pointer; \
             font-size: {size}px;",
            bg = COLOR_ACCENT_PRIMARY,
            fg = COLOR_TEXT_ON_CHROME,
            r = RADIUS_SM,
            touch = TOUCH_MIN,
            ph = SPACE_3,
            size = FONT_SIZE_BODY,
        )
    } else {
        format!(
            "background: {bg}; color: {fg}; border: none; border-radius: {r}px; \
             min-width: {touch}px; min-height: {touch}px; cursor: pointer; \
             font-size: 18px; display: flex; align-items: center; \
             justify-content: center;",
            bg = if hovered() {
                COLOR_ACCENT_PRIMARY_HOVER
            } else {
                COLOR_ACCENT_PRIMARY
            },
            fg = COLOR_TEXT_ON_CHROME,
            r = RADIUS_SM,
            touch = TOUCH_MIN,
        )
    };

    rsx! {
        button {
            // The accessible name, in **both** forms. A tooltip is a hover
            // affordance and never the accessible name — I-02's "independent
            // accessible label".
            "aria-label": label.clone(),
            style: button_style,
            onmounted: move |evt: MountedEvent| { trigger.set(Some(evt)); },
            onmouseenter: move |_| {
                // The pointer observation, at the surface that consumes it.
                crate::note_pointer(PointerPrecision::Fine);
                hovered.set(true);
                let Some(evt) = trigger.peek().clone() else {
                    return;
                };
                spawn(async move {
                    if let Ok(r) = evt.get_client_rect().await {
                        tooltip_anchor.set(Some(Rect {
                            x: r.origin.x as f32,
                            y: r.origin.y as f32,
                            width: r.size.width as f32,
                            height: r.size.height as f32,
                        }));
                    }
                });
            },
            onmouseleave: move |_| { hovered.set(false); },
            ontouchstart: move |_| {
                // A touch is the only positive evidence of a coarse pointer, and
                // it must be recorded even though this button's own presentation
                // already defaults to the labelled form: `Unknown` and `Coarse`
                // agree here and disagree elsewhere (T4.4, T7.1).
                crate::note_pointer(PointerPrecision::Coarse);
            },
            onclick: move |_| { props.on_click.call(()); },
            if labelled { "{label}" } else { "＋" }
        }
    }
}

/// The Recent section's heading row: the section title and the Open action.
///
/// Lives here rather than in `mod.rs` because the two are one decision — moving
/// the action out of the list's foot and into its heading is what T4.3 *is* —
/// and because `mod.rs` is at the file ceiling.
pub(super) fn recent_heading(
    title: String,
    open_label: String,
    on_open: impl FnMut(()) + 'static,
) -> Element {
    rsx! {
        div {
            style: format!(
                "display: flex; align-items: center; justify-content: \
                 space-between; gap: {gap}px; margin-bottom: {mb}px;",
                gap = SPACE_2,
                mb = SPACE_2,
            ),
            h2 {
                style: format!(
                    "font-size: {size}px; color: {fg}; margin: 0; \
                     font-weight: {weight};",
                    size = FONT_SIZE_BODY,
                    fg = crate::tokens::colors::COLOR_TEXT_ON_CHROME_SECONDARY,
                    weight = crate::tokens::typography::FONT_WEIGHT_SEMIBOLD,
                ),
                "{title}"
            }
            AtOpenAction { label: open_label, on_click: on_open }
        }
    }
}

/// The tooltip's own box.
fn tooltip_content(label: String) -> Element {
    rsx! {
        div {
            style: format!(
                "background: {bg}; color: {fg}; border-radius: {r}px; \
                 padding: {pv}px {ph}px; font-size: {size}px; \
                 width: 100%; box-sizing: border-box;",
                bg = COLOR_SURFACE_CHROME,
                fg = COLOR_TEXT_ON_CHROME,
                r = RADIUS_SM,
                pv = SPACE_2,
                ph = SPACE_3,
                size = FONT_SIZE_LABEL,
            ),
            "{label}"
        }
    }
}

#[cfg(test)]
#[path = "open_button_tests.rs"]
mod tests;
