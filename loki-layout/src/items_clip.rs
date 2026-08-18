// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 AppThere Loki contributors

//! Where a split paragraph's clip edge lands, and what that costs.
//!
//! One module so the two halves of the contract sit together: the floor the
//! renderers apply, and the slack decoration placement must therefore keep.
//! They were previously a constant in `items.rs` and a `.floor()` buried in
//! the flow engine, which is how they drifted apart.

/// How much of a split fragment's bottom edge may be shaved by clip rounding, in
/// points.
///
/// A paragraph split across a page or column boundary is emitted as a
/// [`PositionedItem::ClippedGroup`] whose height is the **exact** line boundary
/// (`flow_split::emit_fragment`). The rounding happens at paint time instead:
/// each renderer floors the clip's bottom edge to a whole device pixel
/// ([`clip_bottom_device_px`]), because a fractional edge times the scale rounds
/// up one physical pixel and leaks the next line's top row — and only the
/// renderer knows the scale it is painting at.
///
/// One device pixel is at most one point (the renderers are never asked to paint
/// below 1.0 scale), so this is the bound on what the paint-time floor can take.
/// It matters because the "flooring never clips visible ink" argument is true of
/// glyphs — which stop short of the line box bottom — and **false of
/// decorations**. A spelling squiggle is anchored below the descender, and with
/// an exact line height there is no leading between the descender and the box
/// bottom, so the band crossed the boundary and was cut on both sides of it
/// (Spec 08 I-06).
///
/// Anything that must stay visible therefore has to sit at least this far above
/// the line box bottom. Consumers: [`clip_bottom_device_px`] applies the floor,
/// `para_underlays::emit_spelling_squiggles` respects it.
pub const FRAGMENT_CLIP_FLOOR_SLACK_PT: f32 = 1.0;

/// The device-space bottom edge a [`PositionedItem::ClippedGroup`] must be
/// painted with: `(max_y + offset) * scale`, floored to a whole device pixel.
///
/// Every renderer routes its clip through here rather than rounding for itself,
/// so the "does the clip leak the next line" question has one answer. Flooring
/// (not rounding) is the point: rounding up admits one more physical row, which
/// is exactly the next line's top edge. See [`FRAGMENT_CLIP_FLOOR_SLACK_PT`] for
/// what that costs and who compensates.
#[inline]
#[must_use]
pub fn clip_bottom_device_px(max_y: f32, offset_y: f32, scale: f32) -> f32 {
    ((max_y + offset_y) * scale).floor()
}

#[cfg(test)]
#[path = "items_clip_tests.rs"]
mod tests;
