// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 AppThere Loki contributors

//! The fallback, asserted at the conditions that reach it.
//!
//! Both polarities per L08-045, and the reachability cases stated in the units
//! that make them reachable — a text-size setting and a wrapped name, not a
//! pathological rect.

use super::super::geometry::{place, Align, PlacementRequest, Rect, Side};
use super::{present, Presentation, MIN_ANCHORED_HEIGHT_PX};

/// A phone in landscape with the soft keyboard up: a ~360 dp window, less a ~30
/// px top inset and a ~180 px IME (which the platform adds to the **bottom**
/// inset — see `safe_area`).
const LANDSCAPE_IME_VIEWPORT: Rect = Rect {
    x: 0.0,
    y: 30.0,
    width: 640.0,
    height: 150.0,
};

/// The tile's fixed parts, from `template_gallery.rs`: 12 padding + 72
/// thumbnail + 8 gap + 12 padding. Only the label term varies.
const TILE_FIXED_PX: f32 = 104.0;

/// A tile whose label occupies `label_px`, anchored at the top of `viewport`.
fn tile(label_px: f32, viewport: Rect) -> PlacementRequest {
    PlacementRequest {
        anchor: Rect::new(50.0, viewport.y + 10.0, 100.0, TILE_FIXED_PX + label_px),
        width: 300.0,
        height: 320.0,
        viewport,
        preferred: Side::Below,
        align: Align::Start,
        gap: 4.0,
        margin: 8.0,
    }
}

/// **The reachability table, executable — and it has two thresholds, not one.**
///
/// The bound was first derived with the label at nominal size against the
/// *blank box* condition (no room at all): `anchor >= viewport - 2*(gap+margin)`
/// = 126px here, which a 120px tile clears by six. Only the label scales with
/// the user's text-size setting, and Android's runs to 2.0 — so putting that
/// term at maximum, as the honest derivation must, crosses it:
///
/// | label | tile | ≥ 126? |
/// | --- | ---: | --- |
/// | 16px — one line at 1.0× | 120 | no, by 6 |
/// | 24px — one line at 1.5× | 128 | **yes** |
/// | 32px — one line at 2.0×, or two lines at 1.0× | 136 | **yes** |
/// | 64px — two lines at 2.0× | 168 | **yes** |
///
/// A wrapped template name reaches it at nominal size with no accessibility
/// setting at all.
///
/// **And the usable threshold is earlier still.** Requiring one touch target
/// rather than one pixel moves the bound to `anchor >= viewport - 24 - 44` =
/// 82px, which *every* row above crosses — the nominal tile by 38. So the
/// font-scale progression is what turns the menu into a blank box, and the
/// menu was already unusable at 1.0×. Asserted here in both forms so a later
/// reader cannot conclude the accessibility setting is what causes the problem;
/// it is what makes the problem invisible.
#[test]
fn a_tile_in_a_landscape_keyboard_viewport_is_modal_at_every_text_size() {
    let blank_box_threshold = LANDSCAPE_IME_VIEWPORT.height - 2.0 * (4.0 + 8.0);
    assert!((blank_box_threshold - 126.0).abs() < 0.001);
    let cases = [
        (16.0_f32, "one line at 1.0x — the nominal derivation", false),
        (24.0, "one line at 1.5x font scale", true),
        (32.0, "one line at 2.0x, or two lines at 1.0x", true),
        (64.0, "two lines at 2.0x", true),
    ];
    for (label, why, blank) in cases {
        let req = tile(label, LANDSCAPE_IME_VIEWPORT);
        assert_eq!(
            req.anchor.height >= blank_box_threshold,
            blank,
            "{why}: a {}px tile against the {blank_box_threshold}px blank-box \
             threshold",
            req.anchor.height,
        );
        assert_eq!(
            present(req),
            Presentation::Modal,
            "{why}: the usable threshold is 44px earlier than the blank-box one, \
             so this is modal whether or not it would have been blank",
        );
    }
}

/// The nominal tile is the case the first derivation called safe. It clears the
/// **blank box** threshold by six pixels and misses the **usable** one by
/// thirty-eight — which is the whole correction in one fixture.
#[test]
fn the_nominal_tile_clears_the_blank_box_bound_and_fails_the_usable_one() {
    let req = tile(16.0, LANDSCAPE_IME_VIEWPORT);
    assert!(
        (req.anchor.height - 120.0).abs() < 0.001,
        "the nominal tile is no longer 120px: {}",
        req.anchor.height,
    );
    let placed = place(req);
    assert!(
        placed.rect.height > 0.0,
        "not a blank box: {:?}",
        placed.rect,
    );
    assert!(
        placed.rect.height < MIN_ANCHORED_HEIGHT_PX,
        "but smaller than one touch target: {}px",
        placed.rect.height,
    );
    assert_eq!(present(req), Presentation::Modal);
}

/// The other polarity (L08-045): an ordinary anchor on an ordinary screen stays
/// anchored, so `present` cannot be a constant `Modal` that quietly sends every
/// menu full-screen.
#[test]
fn an_ordinary_anchor_stays_anchored() {
    let vp = Rect::new(0.0, 0.0, 900.0, 700.0);
    let req = tile(16.0, vp);
    let Presentation::Anchored(p) = present(req) else {
        panic!("a 120px tile in a 700px viewport must anchor");
    };
    assert!(p.rect.is_inside(vp));
    assert!(p.rect.height >= MIN_ANCHORED_HEIGHT_PX);
}

/// **The threshold is one touch target, and it is asserted as one.** A menu that
/// can show 43px cannot present a single WCAG 2.5.8 target, so it is not a
/// cramped menu — it is one the house standard does not permit.
#[test]
fn an_overlay_shorter_than_one_touch_target_is_modal() {
    // A viewport sized so the room below the anchor lands just under, then just
    // over, the touch minimum.
    for (room, expect_modal) in [
        (MIN_ANCHORED_HEIGHT_PX - 1.0, true),
        (MIN_ANCHORED_HEIGHT_PX, false),
    ] {
        let vp = Rect::new(0.0, 0.0, 900.0, 100.0 + room + 4.0 + 8.0);
        let mut req = tile(16.0, vp);
        // Anchor ending at y=100, so the room below is exactly `room`.
        req.anchor = Rect::new(50.0, 0.0, 100.0, 100.0);
        let got = present(req);
        assert_eq!(
            got == Presentation::Modal,
            expect_modal,
            "with {room}px of room and a {MIN_ANCHORED_HEIGHT_PX}px minimum, got \
             {got:?}",
        );
    }
}
