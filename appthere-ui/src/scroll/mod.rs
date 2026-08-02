// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 AppThere Loki contributors

//! Scroll observation and control for a document container (Spec 08 Phase 1).
//!
//! # Why this lives in `appthere-ui`
//!
//! Every AppThere app has a scrollable document surface with the same three
//! needs: know where it is, know what is visible, and put something on screen.
//! Before this module those were three separate ad-hoc answers inside
//! `loki-text`. Per L08-005, shared UI primitives live here.
//!
//! # What Blitz gives us, and what it does not
//!
//! Spike S0.1 (`docs/spikes/S0.1-blitz-scroll-capability.md`) found that five
//! of the six scroll capabilities Spec 08 needs already ship in the vendored
//! patch set:
//!
//! - reading the offset, and subscribing to changes — the PATCH(loki) chain
//!   `scroll_node_by_collect` → `Document::handle_scroll_changes` → a DOM
//!   `scroll` event, mirrored into [`ScrollMetrics`];
//! - the visible rect — the same event, plus `MountedData::get_client_rect`;
//! - instant programmatic scroll — `MountedData::scroll`;
//! - absolute-positioned overlays — already proven by the spelling popup.
//!
//! The missing one is **animated** programmatic scroll: `MountedData::scroll`
//! ignores its `ScrollBehavior` and `scroll_to` is a no-op. Per S0.1 that is
//! closed app-side, in [`animate`] and [`ViewportController::animate`], rather
//! than with another Blitz patch.
//!
//! # Shape
//!
//! | Module | Role |
//! | --- | --- |
//! | [`metrics`] | live geometry of the container |
//! | [`reveal`] | pure "what offset shows this rect" arithmetic |
//! | [`animate`] | easing + the motion preference |
//! | [`controller`] | the Dioxus-facing handle |
//!
//! The arithmetic is deliberately separate from the Dioxus surface so it is
//! unit-tested without a window — the same split `responsive::page_fit` uses.

mod animate;
mod controller;
mod metrics;
mod reveal;
mod zoom_anchor;

pub use animate::{animation_step, ease_out_cubic, MotionPreference};
pub use controller::{use_viewport_controller, ContentRect, ViewportController};
pub use metrics::ScrollMetrics;
pub use reveal::{reveal_offset, RevealMargin, CARET_LEADING_LINES, CARET_TRAILING_LINES};
pub use zoom_anchor::{anchored_scroll, ZoomAnchor};
