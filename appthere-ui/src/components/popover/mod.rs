// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 AppThere Loki contributors

//! The shared overlay primitive (Spec 08 T4.1).
//!
//! # Invariant: the component wires, it does not decide
//!
//! Every decision lives in one of four pure modules — [`geometry`] (placement,
//! flip, shift, clamp), [`interaction`] (key routing, focus target),
//! [`interaction_anchor`](interaction::anchor) (scroll and resize response) and
//! [`dismiss_order`] (the order focus and unmount happen in). **Arithmetic or key
//! matching appearing in the component is a signal that a module is missing a
//! case, not that the component needs logic.**
//!
//! That is what stops the four consumers — T4.2's entry menu, T5.2's colour
//! picker, T5.4's zoom popover, T7.1's status overflow — from each growing their
//! own variant of a decision already made, which is the failure the
//! "scope for four consumers, not two" instruction guards against.
//!
//! It is checkable by reading: the component should be short enough that its
//! brevity is itself the check.
//!
//! # Two hazards the pure modules cannot see
//!
//! - **Reposition loops.** [`interaction::on_anchor_change`] compares rects for
//!   exact float equality, which is correct and would loop if layout ever
//!   returned a sub-pixel-different rect for an unmoved anchor. It **warns in
//!   production** after `REPOSITION_BURST_WARN` consecutive repositions, because
//!   the cause is sub-pixel jitter on real re-renders and a test-only assertion
//!   is an instrument that speaks only where the hazard is not. Tests also assert
//!   [`interaction::repositions`] does not advance across idle frames.
//! - **The anchor is not "outside".** [`wiring::is_outside_dismiss`] excludes it,
//!   or clicking the trigger dismisses and the trigger's own handler reopens —
//!   the control stops toggling and both handlers look correct.
//! - **One popover at a time.** [`wiring::open_response`] enforces it, since the
//!   reposition counter is process-wide and a focus restoration needs one
//!   candidate anchor. Opening a second dismisses the first.
//! - **Dismissal ordering.** Focus must move *before* the popover unmounts —
//!   see [`dismiss_order`], which returns the sequence rather than leaving the
//!   order to whichever line was typed first.

pub mod dismiss_order;
pub mod geometry;
pub mod interaction;
pub mod wiring;

pub use geometry::{place, Align, Placement, PlacementRequest, Rect, Side};
