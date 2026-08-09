// SPDX-License-Identifier: Apache-2.0

//! Layout facts about the editor's scroll container that more than one layer
//! needs (Spec 08 r67).

use appthere_ui::tokens;

/// Fallback viewport height (CSS px) for tile virtualization before the scroll
/// container is first measured. A named default — not a hardcoded screen
/// dimension assumed in a layout path (cf. the 1280px viewport bug, Spec 01
/// audit A-1) — used only for the single frame until `get_client_rect` reports
/// the real height.
pub(super) const DEFAULT_VIEWPORT_HEIGHT_PX: f64 = 800.0;

/// The scroll container's vertical padding, in CSS px — **one fact, one source**.
///
/// # Why this is a constant and not four `tokens::SPACE_6`
///
/// Scroll position 0 is the container's padding edge; page 0 starts one padding
/// below it. Four separate consumers need that number and each read
/// `tokens::SPACE_6` directly: the CSS that creates the padding, the caret
/// reveal's `content_top_px`, the renderer's `content_padding_bottom_px`, and —
/// as of r67 — the residency plan's `content_padding_top_px`, whose absence was
/// letting a page with a sliver on screen be rasterised at half scale while its
/// neighbour stayed sharp.
///
/// Four copies of one layout fact is the shape L08-029 says drifts, and the
/// drift here is silent: change the CSS alone and the plan quietly measures
/// visibility from the wrong origin again, with no test and no gate to notice.
/// Reading the token here and only here makes the CSS and the model move
/// together by construction.
///
/// It lives in its own module rather than in `editor_canvas` because that file
/// is over the 300-line ceiling and may not grow.
pub(super) const CANVAS_CONTENT_PADDING_PX: f32 = tokens::SPACE_6;
