// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 AppThere Loki contributors

//! The overflow menu's size and placement, and the role decision under them.

use super::{overflow_menu_placement, overflow_menu_size};
use crate::components::popover::{route_key, Align, Key, KeyAction, Rect, Role, Side};
use crate::components::ribbon::groups::RibbonGroupSpec;
use crate::responsive::GroupMetrics;

fn spec(full_px: f32) -> RibbonGroupSpec {
    RibbonGroupSpec {
        metrics: GroupMetrics {
            priority: 1,
            full_px,
            condensed_px: full_px / 2.0,
        },
        label: None,
        aria_label: format!("group-{full_px}"),
        content: dioxus::prelude::VNode::empty(),
    }
}

/// The trigger's rect, as `get_client_rect` would report it for a More button
/// at the right-hand end of a ribbon strip near the window bottom.
const TRIGGER: Rect = Rect {
    x: 1180.0,
    y: 700.0,
    width: 44.0,
    height: 44.0,
};

/// **The width comes from the groups**, not from a constant. `full_px` is the
/// same measurement the cascade used to decide these groups did not fit, read a
/// second time — a magic width here would be a second statement of how wide a
/// group is, and the two would disagree the first time a tab declared an
/// unusual one.
#[test]
fn the_width_follows_the_widest_group() {
    let narrow = [spec(200.0)];
    let wide = [spec(200.0), spec(480.0)];
    let (w_narrow, _) = overflow_menu_size(&narrow.iter().collect::<Vec<_>>());
    let (w_wide, _) = overflow_menu_size(&wide.iter().collect::<Vec<_>>());
    assert!(
        w_wide > w_narrow,
        "a wider group must widen the menu: {w_narrow} vs {w_wide}",
    );
    assert!(w_wide >= 480.0, "the widest group must fit: {w_wide}");
}

/// **And the height follows the count.** Without this the size function passes
/// for one that ignores everything but the widest group, and a five-group menu
/// would be asked to fit in one group's height.
#[test]
fn the_height_follows_the_group_count() {
    let one = [spec(200.0)];
    let three = [spec(200.0), spec(200.0), spec(200.0)];
    let (_, h_one) = overflow_menu_size(&one.iter().collect::<Vec<_>>());
    let (_, h_three) = overflow_menu_size(&three.iter().collect::<Vec<_>>());
    assert!(h_three > h_one * 2.0, "{h_one} vs {h_three}");
}

/// A menu too narrow to hold a control would be unusable — indistinguishable
/// from the defect this task fixes.
///
/// **Asserted against a usable width, not against zero.** The first draft asked
/// only for `w > 0.0`, which the menu's own padding satisfies: removing the
/// floor entirely left the test passing on 16 px of padding around nothing. A
/// test that a mutation cannot kill is describing the code, not checking it.
#[test]
fn the_menu_is_never_degenerate() {
    let usable = 100.0;
    let (w, h) = overflow_menu_size(&[]);
    assert!(w >= usable && h > 0.0, "empty menu is {w}x{h}");
    let tiny = [spec(0.0)];
    let (w, _) = overflow_menu_size(&tiny.iter().collect::<Vec<_>>());
    assert!(w >= usable, "a zero-width group gave a {w}px menu");
}

/// **Above and end-aligned**, both deliberate: the ribbon is at the window
/// bottom, and the More button is the last thing in the strip. Asking for
/// `Below`/`Start` and relying on the primitive's flip and shift would make the
/// correction path the one every open takes.
#[test]
fn the_menu_opens_upward_from_the_strips_right_edge() {
    let p = overflow_menu_placement(TRIGGER, (300.0, 200.0));
    assert_eq!(p.preferred, Side::Above);
    assert_eq!(p.align, Align::End);
    assert_eq!(p.anchor, TRIGGER);
}

/// The requested size reaches the placement — a placement that discarded it
/// would be positioned correctly and sized wrong, which reads as a clipped menu.
#[test]
fn the_size_reaches_the_placement() {
    let p = overflow_menu_placement(TRIGGER, (321.0, 234.0));
    assert!((p.width - 321.0).abs() < f32::EPSILON);
    assert!((p.height - 234.0).abs() < f32::EPSILON);
}

/// **`Role::Panel` passes the keys a button container needs**, which is why it
/// is not `Role::Menu`: arrows, Enter/Space and characters reach whichever
/// button has focus rather than being swallowed into `Next`/`Prev` that this
/// content has no item model to perform.
#[test]
fn the_panel_role_passes_keys_to_the_focused_button() {
    for key in [Key::Down, Key::Up, Key::Activate, Key::Char('b')] {
        assert_eq!(
            route_key(Role::Panel, key),
            KeyAction::PassThrough,
            "{key:?} must reach the focused control",
        );
    }
    // And Escape still closes, which is the host's to perform.
    assert_eq!(route_key(Role::Panel, Key::Escape), KeyAction::Dismiss);
}

/// **The strain this migration exposed, stated as a test so it cannot be
/// forgotten.** `Tab` in a panel is `FocusNextControl`, and nothing in the
/// workspace performs it — `set_focus` is a `bool`, so "focus the next control"
/// cannot be expressed. This menu's `on_key` therefore dismisses on it, because
/// a key consumed and dropped would trap the keyboard inside a menu it could
/// not leave.
///
/// When the mechanism lands (the `advance_focus_past|focus_next_node` register
/// row), this test is the reminder that a real performer belongs here.
#[test]
fn tab_is_a_focus_move_this_content_cannot_perform() {
    assert_eq!(
        route_key(Role::Panel, Key::Tab),
        KeyAction::FocusNextControl
    );
    assert_eq!(
        route_key(Role::Panel, Key::ShiftTab),
        KeyAction::FocusPrevControl
    );
}
