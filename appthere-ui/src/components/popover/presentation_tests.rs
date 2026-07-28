// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 AppThere Loki contributors

//! The fallback, asserted at the conditions that reach it.
//!
//! Both polarities per L08-045, and the reachability cases stated in the units
//! that make them reachable — a text-size setting and a wrapped name, not a
//! pathological rect.

use super::super::geometry::{place, Align, PlacementRequest, Rect, Side};
use super::{present, Presentation, MIN_ANCHORED_HEIGHT_PX, MIN_ANCHORED_MENU_PX};

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
        min_anchored_height: MIN_ANCHORED_MENU_PX,
    }
}

/// **The reachability table, executable — and it has two thresholds, not one.**
///
/// The bound was first derived with the label at nominal size against the
/// *blank box* condition (no room at all): `anchor >= viewport - 2*(gap+margin)`
/// = 126px here, which a 120px tile clears by six. The label is the one term
/// that would scale with the user's text-size setting, and Android's runs to
/// 2.0 — so the honest derivation puts it at maximum:
///
/// | label | tile | ≥ 126? | reachable today? |
/// | --- | ---: | --- | --- |
/// | 16px — one line at 1.0× | 120 | no, by 6 | — |
/// | 24px — one line at 1.5× | 128 | **yes** | **only once I-24 lands** |
/// | 32px — one line at 2.0× | 136 | **yes** | **only once I-24 lands** |
/// | 32px — **two lines at 1.0×**, a name that wraps | 136 | **yes** | **yes, today** |
/// | 64px — two lines at 2.0× | 168 | **yes** | only once I-24 lands |
///
/// # The font-scale rows describe a future state, deliberately
///
/// **Loki does not apply any platform's text-size setting today** (I-24, settled
/// from source: no font-scale term exists in `Viewport`, and the type tokens are
/// absolute px constants). So a reader who takes those rows as current behaviour
/// will go looking for a blank box they cannot reproduce. They are kept because
/// I-24 is a defect to fix rather than a decision to keep, and this is the table
/// that says what fixing it exposes.
///
/// **The wrapped-name row needs no accessibility setting and fires today** — it
/// is the one that makes this a present-tense case rather than a forecast.
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

/// **N is two rows, and that is a decision rather than an inheritance.**
///
/// One touch target is the floor — it bounds *unusable*. It is a poor place to
/// stay anchored: a 44px window over a four-action menu shows one item with no
/// sign that the others exist, so the user does not scroll and never learns what
/// was there. The second row is the affordance that makes a list read as a list.
///
/// This is the band the decision lives in: room for one row but not two.
#[test]
fn a_menu_with_room_for_one_row_but_not_two_is_modal() {
    let room = MIN_ANCHORED_HEIGHT_PX + 1.0; // 45px: one row fits, two do not.
    assert!(
        room >= MIN_ANCHORED_HEIGHT_PX && room < MIN_ANCHORED_MENU_PX,
        "fixture must sit between the floor and the menu minimum: {room}",
    );
    let req = req_with_room_below(room, MIN_ANCHORED_MENU_PX);
    assert_eq!(
        present(req),
        Presentation::Modal,
        "one row is legal and not worth anchoring — the modal shows the whole \
         menu at once",
    );
    // And two rows is enough to stay.
    let req = req_with_room_below(MIN_ANCHORED_MENU_PX, MIN_ANCHORED_MENU_PX);
    assert!(matches!(present(req), Presentation::Anchored(_)));
}

/// **The floor holds against a consumer that asks for less.** A request of `0.0`
/// is not a way back to the pre-r48 behaviour: a sub-touch-target menu is not
/// permitted whoever asks for it (L08-043).
#[test]
fn a_consumer_asking_below_the_floor_still_gets_the_floor() {
    let just_under = req_with_room_below(MIN_ANCHORED_HEIGHT_PX - 1.0, 0.0);
    assert_eq!(
        present(just_under),
        Presentation::Modal,
        "43px is below one touch target, and asking for 0 does not license it",
    );
    let at_the_floor = req_with_room_below(MIN_ANCHORED_HEIGHT_PX, 0.0);
    assert!(
        matches!(present(at_the_floor), Presentation::Anchored(_)),
        "but the floor itself is honoured — a consumer that genuinely wants one \
         row gets one row",
    );
}

/// A request whose room below the anchor is exactly `room`, asking for
/// `min_anchored` as its minimum.
fn req_with_room_below(room: f32, min_anchored: f32) -> PlacementRequest {
    let mut req = tile(16.0, Rect::new(0.0, 0.0, 900.0, 100.0 + room + 4.0 + 8.0));
    req.anchor = Rect::new(50.0, 0.0, 100.0, 100.0);
    req.min_anchored_height = min_anchored;
    req
}
