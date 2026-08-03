// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 AppThere Loki contributors

//! The zoom control: out / readout / in, with a preset menu (Spec 08 T5.4).
//!
//! # It shows the *requested* zoom, and says so when that is not what you get
//!
//! Requirement 6. The readout is the zoom the reader asked for, never the
//! capability-capped effective one, because a control that displays the reduced
//! figure erodes intent a nudge at a time: each adjustment starts from the
//! smaller number and the original setting is gone after three presses. The
//! indicator — [`zoom_is_capped`] — is what keeps that display honest.
//!
//! # The menu is hosted, not rendered in place
//!
//! The status bar is chrome with its own overflow, so an absolutely-positioned
//! popup inside it is clipped exactly as T4.2's menu was. It goes through
//! `AtPopoverHost` with [`Role::Menu`], which is also what gives it a keyboard
//! for free (T4.5).

mod field;
mod menu;
mod menu_keys;
mod rows;

use dioxus::prelude::*;

pub use rows::{zoom_rows, ZoomCommands, ZoomRow};

use menu::{zoom_menu_request, ZoomMenuContext};

use super::popover::{use_popover_anchor, PopoverId, Rect};
use super::zoom::{next_zoom, prev_zoom, zoom_is_capped};
use crate::tokens::colors::{COLOR_TEXT_ON_CHROME, COLOR_TEXT_ON_CHROME_SECONDARY};
use crate::tokens::spacing::{RADIUS_SM, SPACE_1, SPACE_2, TOUCH_MIN};
use crate::tokens::typography::FONT_SIZE_LABEL;
use crate::{use_safe_area, use_window_size};

/// Identifies the zoom menu to the popover singleton rule.
const ZOOM_POPOVER_ID: PopoverId = PopoverId(0x0_2004);

/// Translated labels. Passed in, because `appthere_ui` is i18n-agnostic.
#[derive(Clone, PartialEq)]
pub struct AtZoomLabels {
    /// Accessible name of the zoom-out button.
    pub zoom_out: String,
    /// Accessible name of the zoom-in button.
    pub zoom_in: String,
    /// Accessible name of the readout button that opens the menu.
    pub menu: String,
    /// "Fit width" row label.
    pub fit_width: String,
    /// "Fit page" row label.
    pub fit_page: String,
    /// "Actual size" row label.
    pub actual_size: String,
    /// Shown when a capability limit is holding the zoom below the request.
    pub reduced_note: String,
}

/// Props for [`AtZoomControl`].
#[derive(Props, Clone, PartialEq)]
pub struct AtZoomControlProps {
    /// The zoom the user asked for, in percent. **Not** the effective zoom.
    pub percent: u32,
    /// The capability cap in force, in permille, if any.
    #[props(default)]
    pub capability_limit_permille: Option<u16>,
    /// Which computed rows this app can service.
    #[props(default)]
    pub commands: ZoomCommands,
    /// Translated labels.
    pub labels: AtZoomLabels,
    /// A new requested zoom, in percent.
    pub on_change: EventHandler<u32>,
    /// Fit-width chosen; the app computes the percent from its own measurements.
    pub on_fit_width: EventHandler<()>,
    /// Fit-page chosen.
    pub on_fit_page: EventHandler<()>,
    /// Actual-size chosen.
    pub on_actual_size: EventHandler<()>,
}

/// The width this control occupies in the status bar (CSS px).
///
/// Three [`TOUCH_MIN`] controls with [`SPACE_1`] between them — the same numbers
/// the rsx below applies, stated once so the status bar's fit engine
/// (Spec 08 T7.1) can charge for it without a second, drifting copy.
///
/// It exists because the first version of that engine declared this control by
/// its *readout text*, as if it were a label. That under-declared it by roughly
/// 80 px, the bar overflowed its window at 420 px instead of dropping an item,
/// and the overflow trigger was pushed off the right edge — the exact
/// under-estimating failure the engine's own docs warn about, committed against
/// the one control that can never drop to make room.
pub(crate) const ZOOM_CONTROL_WIDTH_PX: f32 = 3.0 * TOUCH_MIN + 2.0 * SPACE_1;

/// Zoom out / readout / zoom in, with a preset menu on the readout.
///
/// # Touch target
///
/// Each of the three controls is **[`TOUCH_MIN`] wide (44 px) × the full status
/// bar height (24 px)** — not 44 × 44.
///
/// This is the deviation the status bar already makes for its other badges, and
/// it is deliberate rather than overlooked: the bar is 24 px by design, so a
/// 44 px-tall control does not shrink the bar, it **overflows upward and paints
/// over the ribbon**. The first screen sitting showed exactly that. 44 × 24
/// still clears WCAG 2.5.8's 24 × 24 minimum; it does not meet 2.5.5's 44 × 44
/// AAA target, which no control in a 24 px bar can.
///
/// The honest fix is a taller status bar on coarse pointers, which is T7.1's
/// compact posture — noted here so this reads as a known bound rather than as
/// the convention having been forgotten.
#[component]
pub fn AtZoomControl(props: AtZoomControlProps) -> Element {
    let popover = use_popover_anchor(ZOOM_POPOVER_ID);
    let window = use_window_size();
    let insets = use_safe_area();
    let mut open = use_signal(|| false);
    let active = use_signal(|| Option::<String>::None);
    // The typed zoom, once the reader has started one. `None` = field hidden.
    let typed = use_signal(|| Option::<String>::None);
    let mut anchor = use_signal(|| Option::<MountedEvent>::None);
    let mut anchor_rect = use_signal(|| Option::<Rect>::None);

    // The menu request, pushed from an effect rather than a render — the shape
    // every popover consumer uses, so opening is a state change and the host
    // owns the mounting.
    {
        let ctx = ZoomMenuContext {
            percent: props.percent,
            commands: props.commands,
            labels: props.labels.clone(),
            on_change: props.on_change,
            on_fit_width: props.on_fit_width,
            on_fit_page: props.on_fit_page,
            on_actual_size: props.on_actual_size,
        };
        use_effect(move || {
            let Some(popover) = popover else { return };
            let Some(rect) = *anchor_rect.read() else {
                return;
            };
            if !open() {
                popover.dismiss();
                return;
            }
            popover.open(
                zoom_menu_request(
                    ctx.clone(),
                    rect,
                    anchor.peek().as_ref().map(|e: &MountedEvent| e.data()),
                    active,
                    open,
                    typed,
                ),
                window,
                insets,
            );
        });
    }

    let capped = zoom_is_capped(props.percent, props.capability_limit_permille);
    let readout = format!("{}%", props.percent);

    // `height: 100%`, not `min-height: TOUCH_MIN` — see the touch-target note on
    // the component. A 44px-tall button in a 24px bar overflows it upward and
    // paints over the ribbon, which is what the first screen sitting showed.
    let btn = |extra: &str| {
        format!(
            "background: transparent; border: none; color: {fg}; cursor: pointer; \
             min-width: {t}px; height: 100%; box-sizing: border-box; padding: 0; \
             display: flex; align-items: center; justify-content: center; \
             flex-shrink: 0; border-radius: {r}px; font-size: {s}px; {extra}",
            fg = COLOR_TEXT_ON_CHROME,
            t = TOUCH_MIN,
            r = RADIUS_SM,
            s = FONT_SIZE_LABEL,
        )
    };

    rsx! {
        div {
            style: format!(
                "display: flex; align-items: center; gap: {}px;",
                SPACE_1,
            ),
            button {
                style: btn(""),
                aria_label: props.labels.zoom_out.clone(),
                onclick: move |_| props.on_change.call(prev_zoom(props.percent)),
                "−"
            }
            button {
                style: btn(&format!("padding: 0 {}px;", SPACE_2)),
                aria_label: props.labels.menu.clone(),
                onmounted: move |e: MountedEvent| {
                    let el = e.clone();
                    anchor.set(Some(e));
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
                onclick: move |_| open.toggle(),
                "{readout}"
                // The indicator, not a second number: what the reader needs to
                // know is that the page is rendering below their setting, and a
                // second percent invites the question of which one is real.
                if capped {
                    span {
                        style: format!(
                            "color: {}; margin-left: {}px;",
                            COLOR_TEXT_ON_CHROME_SECONDARY, SPACE_1,
                        ),
                        title: props.labels.reduced_note.clone(),
                        "▼"
                    }
                }
            }
            button {
                style: btn(""),
                aria_label: props.labels.zoom_in.clone(),
                onclick: move |_| props.on_change.call(next_zoom(props.percent)),
                "+"
            }
        }
    }
}
