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
//!   returned a sub-pixel-different rect for an unmoved anchor. A consumer should
//!   assert [`interaction::repositions`] does not advance across idle frames;
//!   a loop otherwise presents as a frame-rate symptom.
//! - **Dismissal ordering.** Focus must move *before* the popover unmounts —
//!   see [`dismiss_order`], which returns the sequence rather than leaving the
//!   order to whichever line was typed first.

pub mod dismiss_order;
pub mod geometry;
pub mod interaction;

pub use geometry::{place, Align, Placement, PlacementRequest, Rect, Side};
