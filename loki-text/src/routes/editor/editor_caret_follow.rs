// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 AppThere Loki contributors

//! Keep the caret on screen while typing (Spec 08 I-05, T1.3–T1.5).
//!
//! # Why a component and not a hook call
//!
//! The effect needs a hook scope, and `render_canvas_area` is a plain function
//! that cannot call hooks. Hosting it in `editor_inner` would grow a file that
//! is already over the 300-line ceiling and pinned by the CI ratchet, so this
//! is a zero-output sensor component mounted inside the canvas subtree instead
//! — the same shape as `SafeAreaResizeSensor` and `AtViewportWidthSensor`, and
//! what ADR-0013 prescribes for anything that needs its own hook scope.
//!
//! # What triggers a reveal (T1.4, L08-019)
//!
//! A reveal fires on a change to the caret's [`CaretRevision`] — its identity —
//! and on nothing else:
//!
//! - typing, deletion and arrow/Home/End navigation move the caret → reveal;
//! - a wheel or touch scroll does not change the caret's identity → no reveal,
//!   so the viewport never fights a user who has scrolled away to read;
//! - drag-select does move the caret, so it is gated explicitly on
//!   `is_dragging`;
//! - clicking to place the caret moves it, but the click was inside the
//!   viewport, so `reveal_offset` finds it already visible and does nothing.
//!
//! ## Why the guard exists, and why subscribing to `cursor_state` was not enough
//!
//! The first version of this file relied on effect subscriptions alone: the
//! effect read `cursor_state` and nothing else, so it looked as though only a
//! caret move could wake it. It also called `ViewportController::scroll_to_reveal`,
//! which read the metrics signal *reactively* — so the effect silently became
//! a subscriber to every scroll event too. Turning the wheel re-ran it, the
//! caret's position relative to the new offset was outside the margin band
//! because the user had just scrolled it there, and the reveal dragged the view
//! back. Wheel scrolling was capped at the margin band around the caret. That
//! is I-20, and it shipped through 31 unit tests, the full workspace suite, the
//! CI clippy command and eight script gates — it took thirty seconds in front
//! of a screen to find.
//!
//! The controller no longer subscribes (see its type docs), which stops the
//! loop. This guard is the second layer: because the trigger is caret identity
//! rather than anything derived from scroll position, a subscription
//! reintroduced by a later edit cannot restart it. Two independent reasons the
//! bug cannot come back, because one of them already looked sufficient.
//!
//! ## What the fix gives up, deliberately
//!
//! Changing zoom, or a relayout that moves the caret's rect while its identity
//! is unchanged, no longer scrolls the caret back into view. Under the buggy
//! version it did — as a side effect of the subscription, not by design. Keeping
//! the caret visible across a zoom change is a reasonable feature, but it is a
//! *different* one, and the only way to get it from here is to reintroduce the
//! recomputation that caused I-20. If it is wanted, add an explicit zoom trigger
//! that reveals once per zoom change; do not widen this effect's subscriptions.
//! The same reasoning covers R25: a caret that stays put while an async font
//! load or image resolution shifts the layout under it will not yank the view.
//!
//! # I-21: the trigger point, and what inspection has already ruled out
//!
//! The reveal fires when `caret_top + caret_height + 3 × line` passes the
//! viewport bottom — that is, when fewer than three line-heights of space
//! remain below the caret's own line. The screen test reports it firing
//! *earlier* than that. T1.9 gives four candidate causes; two are ruled out
//! here, and the `tracing::debug!` below separates the rest in one observation.
//!
//! **Ruled out — the margin arithmetic.** `RevealMargin::caret_lines` is one
//! line leading, three trailing, and `reveal_offset` adds the target's own
//! height before the trailing term. Three clear lines below the caret's line is
//! what the code computes, matching T1.3's wording.
//!
//! **Ruled out — chrome inside `client_height`.** The custom scrollbars are
//! siblings of the scroll container, not children (`editor_canvas`: the
//! vertical bar is beside it in the row, the horizontal bar below it in the
//! column), so neither steals visible height.
//!
//! **Ruled out — a missing zoom factor.** The margin is derived from the caret
//! rect's height, which is already in CSS px at the current zoom, so it scales
//! with zoom by construction rather than by a separate multiply.
//!
//! **Still open, and separated by the log line.**
//!
//! 1. *A fixed pixel term.* `content_top_px` is the container's 24 px top
//!    padding, and the caret rect is offset by it on the assumption that
//!    `scrollTop = 0` sits at the top of the padding box. If Blitz places the
//!    scroll origin after the padding instead, every caret rect is 24 px low
//!    and the trigger comes 24 px early — constant across zoom and font size.
//!    Signature in the log: `caret_bottom` exceeds the caret's true on-screen
//!    position by a constant.
//! 2. *Three lines is simply too generous.* Signature: `caret_bottom` and
//!    `visible_bottom - 3 × line` agree, and the reveal is behaving exactly as
//!    specified. Then it is a taste change and the value moves — but only then.
//!
//! **The value has deliberately not been tuned.** T1.9 is explicit that three
//! of the four causes are bugs, and lowering the constant would mask any of
//! them while making the symptom go away.
//!
//! # The soft keyboard (T1.5) needs no special case
//!
//! T1.5 requires the reveal to target the safe area rather than the window when
//! a soft keyboard is up. It already does, because the reveal measures against
//! the scroll container's **own measured** `client_height` rather than the
//! window: `routes::shell` sizes the shell `calc(100vh - inset_total)`, and the
//! Android inset query folds `WindowInsets.Type.ime()` into that total (S0.4),
//! so the container physically shrinks when the keyboard appears and the
//! measurement follows. Where no IME exists — desktop, or Android with a
//! hardware keyboard — the inset is zero and the safe area *is* the window,
//! which is exactly what T1.5 asks for. Adding a second, explicit safe-area
//! term here would double-count it.
//!
//! # Why there is no debounce timer
//!
//! T1.4 asks for debouncing during fast typing. None is needed, and adding one
//! would make the caret lag the text: `reveal_offset` is idempotent — it
//! returns `None` whenever the caret is already visible with its margins — and
//! when it does scroll it moves the **minimum** distance. Successive keystrokes
//! therefore produce either nothing or a few pixels of follow, never the
//! oscillation a debounce would be protecting against. Revisit if a device says
//! otherwise; it is a one-line change to `scroll_to`'s call site.
//!
//! Note what that argument does *not* cover, and what I-20 proved: idempotence
//! says a reveal is harmless when the caret is already visible. It says nothing
//! about *when* the reveal runs. Not fighting the user is a property of the
//! trigger, not of the reveal — which is why it now lives in the guard above
//! rather than being inferred from `reveal_offset`'s behaviour.

use std::sync::{Arc, Mutex};

use appthere_ui::{RevealMargin, ScrollMetrics, use_viewport_controller};
use dioxus::prelude::*;
use loki_renderer::ViewMode;

use super::editor_caret_follow_geom::{CaretGeometryInput, caret_content_rect};
use super::editor_responsive::zoom_fraction;
use super::editor_scrollbar::CanvasMounted;
use crate::editing::caret_reveal::{CaretRevision, caret_line_height_px, should_reveal};
use crate::editing::cursor::CursorState;
use crate::editing::state::DocumentState;

/// Line height assumed when the caret has no measured rect yet (11 pt single
/// spaced at 100%). Only used for the reveal margin, never for placement.
const FALLBACK_LINE_PX: f32 = 18.0;

#[derive(Clone, Props)]
pub(super) struct CaretFollowProps {
    pub(super) doc_state: Arc<Mutex<DocumentState>>,
    pub(super) cursor_state: Signal<CursorState>,
    pub(super) scroll_metrics: Signal<ScrollMetrics>,
    pub(super) canvas_mounted: CanvasMounted,
    /// True while a pointer drag-select is in progress; suppresses the reveal.
    pub(super) is_dragging: Signal<bool>,
    pub(super) view_mode: Signal<ViewMode>,
    pub(super) zoom_percent: Signal<u32>,
    pub(super) page_gap_px: f32,
    /// Top padding of the scroll container, in CSS px.
    pub(super) content_top_px: f32,
}

impl PartialEq for CaretFollowProps {
    fn eq(&self, other: &Self) -> bool {
        Arc::ptr_eq(&self.doc_state, &other.doc_state)
            && self.cursor_state == other.cursor_state
            && self.scroll_metrics == other.scroll_metrics
            && self.canvas_mounted == other.canvas_mounted
            && self.is_dragging == other.is_dragging
            && self.view_mode == other.view_mode
            && self.zoom_percent == other.zoom_percent
            && self.page_gap_px == other.page_gap_px
            && self.content_top_px == other.content_top_px
    }
}

/// Zero-output sensor that scrolls the caret into view when it moves.
// A Dioxus component must be PascalCase to be usable as `CaretFollow {}` in
// rsx. `#[component]` would supply this allow itself, but it derives the props
// struct from the argument list and so requires every prop to be `PartialEq` —
// `Arc<Mutex<DocumentState>>` is not, hence the hand-written props above with
// their `Arc::ptr_eq` comparison. Same trade as `PageTile` and `ReflowDocView`,
// which are the other two components in the workspace taking a props struct
// directly.
#[allow(non_snake_case)]
pub(super) fn CaretFollow(props: CaretFollowProps) -> Element {
    let mut controller = use_viewport_controller(props.scroll_metrics, props.canvas_mounted);
    let doc_state = Arc::clone(&props.doc_state);
    let cursor_state = props.cursor_state;
    let is_dragging = props.is_dragging;
    let view_mode = props.view_mode;
    let zoom_percent = props.zoom_percent;
    let metrics = props.scroll_metrics;
    let page_gap_px = props.page_gap_px;
    let content_top_px = props.content_top_px;
    // Caret identity at the last reveal. The reveal fires on a change to this
    // and on nothing else (L08-019).
    let mut last_revision = use_signal(|| None::<CaretRevision>);

    use_effect(move || {
        // Subscribes this effect to caret movement.
        let (focus, anchor) = {
            let cs = cursor_state.read();
            (cs.focus.clone(), cs.anchor.clone())
        };
        let Some(focus) = focus else { return };
        // The trigger gate. The effect may re-run for reasons that are not the
        // caret moving; those must not scroll the view. Checked before any
        // geometry work, so a spurious wake is also cheap.
        let revision = CaretRevision::new(focus.clone(), anchor);
        if !should_reveal(last_revision.peek().as_ref(), &revision) {
            return;
        }
        // T1.4: a drag is the user choosing where to look; do not move it.
        if *is_dragging.peek() {
            return;
        }
        let m = *metrics.peek();
        if !m.is_measured() {
            return;
        }

        let rect = caret_content_rect(
            &doc_state,
            &focus,
            CaretGeometryInput {
                view_mode: *view_mode.peek(),
                client_width: m.client_width,
                zoom: zoom_fraction(*zoom_percent.peek()),
                page_gap_px,
                content_top_px,
            },
        );

        let Some(rect) = rect else { return };
        // Record the revision only once the geometry resolved. Recording it on
        // a frame where the layout was stale would mark a caret move as handled
        // without ever revealing it, and the next keystroke at the same
        // position would not retry.
        last_revision.set(Some(revision));
        // T1.3: the margin is three body lines below and one above, measured
        // from the caret's own line so it holds at every size and zoom.
        let line = caret_line_height_px(Some(rect), FALLBACK_LINE_PX);
        // I-21 instrument. The reported symptom — "triggers higher in the
        // viewport than 3 lines" — has four possible causes that a screen test
        // cannot tell apart by eye but these numbers separate immediately:
        // compare `caret_bottom` against `visible_bottom - 3 × line`. If they
        // agree, the margin is doing exactly what it says and 3 lines is simply
        // too generous (a taste change); if they disagree, the shortfall is the
        // bug, and whether it is constant, scales with zoom, or scales with
        // line height names which one. See the module docs.
        tracing::debug!(
            target: "loki_text::caret_follow",
            caret_top = rect.1,
            caret_bottom = rect.1 + rect.3,
            line_px = line,
            scroll_top = m.scroll_top,
            visible_bottom = m.scroll_top + m.client_height,
            trailing_margin_px = line * 3.0,
            "caret reveal evaluated",
        );
        // Instant, always. A smooth caret-follow would lag the text at typing
        // speed; smooth is reserved for discrete jumps (Find, Go To Page).
        controller.scroll_to_reveal(
            rect,
            RevealMargin::caret_lines(line),
            ScrollBehavior::Instant,
        );
    });

    rsx! {}
}
