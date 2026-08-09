// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 AppThere Loki contributors

//! Tests for the status bar's item list and priority order (Spec 08 T7.1).

use super::{build_items, dropped_slots, is_shown, SlotSpec, StatusSlot};
use crate::responsive::resolve_status_fit;

fn spec(slot: StatusSlot, label: &str) -> SlotSpec<'_> {
    SlotSpec {
        slot,
        present: !label.is_empty(),
        label,
    }
}

/// The bar as `loki-text` supplies it, all slots present.
fn full_bar() -> Vec<SlotSpec<'static>> {
    vec![
        spec(StatusSlot::Page, "Page 1 of 4"),
        spec(StatusSlot::WordCount, "1,847 words"),
        spec(StatusSlot::Notice, "1 notice"),
        spec(StatusSlot::StatusNote, "Document saved"),
        spec(StatusSlot::Language, "English (US)"),
        spec(StatusSlot::ViewMode, "Paginated"),
        SlotSpec {
            slot: StatusSlot::Zoom,
            present: true,
            label: "",
        },
        spec(StatusSlot::Collaborators, "2 editing"),
    ]
}

/// **An absent slot is omitted, not zero-width.** A zero-width item would keep a
/// position in the `shown` vector, and every later index would refer to the
/// wrong slot.
#[test]
fn absent_slots_are_left_out_of_the_item_list() {
    let specs = vec![
        spec(StatusSlot::Page, "Page 1 of 4"),
        spec(StatusSlot::WordCount, ""), // absent
        SlotSpec {
            slot: StatusSlot::Zoom,
            present: true,
            label: "",
        },
    ];
    let (items, slots) = build_items(&specs);
    assert_eq!(items.len(), 2, "an absent slot took a position");
    assert_eq!(slots, vec![StatusSlot::Page, StatusSlot::Zoom]);
    assert!(items.iter().all(|i| i.width_px > 0.0));
}

/// **Narrowing drops in the documented order, and never the retained pair.**
/// This walks the width down and records the order slots leave, which is the
/// policy the module table states — a table nothing checked would drift from the
/// code the first time a priority changed.
#[test]
fn items_leave_in_priority_order_and_the_retained_pair_never_does() {
    let specs = full_bar();
    let (items, slots) = build_items(&specs);
    let total: f32 = items.iter().map(|i| i.width_px).sum();

    let mut order_left = Vec::new();
    let mut prev_dropped = 0usize;
    // Step down from "everything fits" to "nothing but the retained pair".
    let mut width = total + 10.0;
    while width > 0.0 {
        let fit = resolve_status_fit(&items, width, prev_dropped);
        for slot in dropped_slots(&slots, &fit.shown) {
            if !order_left.contains(&slot) {
                order_left.push(slot);
            }
        }
        assert!(
            is_shown(&slots, &fit.shown, StatusSlot::Page),
            "the page indicator dropped at {width}px"
        );
        assert!(
            is_shown(&slots, &fit.shown, StatusSlot::Zoom),
            "the zoom control dropped at {width}px"
        );
        prev_dropped = fit.dropped;
        width -= 10.0;
    }

    // Word count and language tie at priority 1, so the tie breaks by layout
    // order: word count sits left of language in `full_bar`.
    assert_eq!(
        order_left,
        vec![
            StatusSlot::WordCount,
            StatusSlot::Language,
            StatusSlot::Collaborators,
            StatusSlot::ViewMode,
            StatusSlot::StatusNote,
            StatusSlot::Notice,
        ],
        "items did not leave in the order the module table states"
    );
}

/// **`is_shown` and "dropped" are different questions.** An absent slot is not
/// shown *and* not dropped — listing it in the overflow would offer a control
/// the app never supplied.
#[test]
fn an_absent_slot_is_neither_shown_nor_dropped() {
    let specs = vec![
        spec(StatusSlot::Page, "Page 1 of 4"),
        spec(StatusSlot::WordCount, ""), // absent
        SlotSpec {
            slot: StatusSlot::Zoom,
            present: true,
            label: "",
        },
    ];
    let (items, slots) = build_items(&specs);
    // A width so small everything droppable would go — there is nothing
    // droppable here, so this is the strongest form of the question.
    let fit = resolve_status_fit(&items, 1.0, 0);
    assert!(!is_shown(&slots, &fit.shown, StatusSlot::WordCount));
    assert!(
        !dropped_slots(&slots, &fit.shown).contains(&StatusSlot::WordCount),
        "an absent slot was listed as dropped"
    );
    // And the guard: a *present* droppable slot does reach the dropped list, so
    // the assertion above is not passing because nothing is ever dropped.
    let (items2, slots2) = build_items(&full_bar());
    let fit2 = resolve_status_fit(&items2, 1.0, 0);
    assert!(
        dropped_slots(&slots2, &fit2.shown).contains(&StatusSlot::WordCount),
        "a present droppable slot never reached the dropped list"
    );
}

/// A chip declares at least a touch-sized hit area even when its label is one
/// character, because that is what it paints. A bare text label does not.
#[test]
fn a_chip_declares_its_hit_area_and_a_label_declares_its_text() {
    let (chip, _) = build_items(&[spec(StatusSlot::ViewMode, "P")]);
    let (text, _) = build_items(&[spec(StatusSlot::WordCount, "P")]);
    assert!(
        chip[0].width_px > text[0].width_px,
        "a one-character chip ({}) declared no more than a one-character label ({})",
        chip[0].width_px,
        text[0].width_px
    );
    // A long label outgrows the minimum, so the `max` is a floor and not a cap.
    let (long, _) = build_items(&[spec(StatusSlot::ViewMode, "Paginated view mode")]);
    assert!(long[0].width_px > chip[0].width_px);
}

/// **The zoom control declares the width it paints, not the width of its text.**
///
/// The first version declared it by its readout ("1000%"), as if it were a
/// label. That under-declared it by roughly 80 px; at a 420 px window the bar
/// overflowed instead of dropping an item and the overflow trigger was pushed
/// off the right edge — photographed under `run.sh statusoverflow` before this
/// existed.
///
/// Under-declaring a **retained** item is the worst case of that error: the item
/// cannot drop to make room, so the whole error lands on the items that can.
#[test]
fn the_zoom_control_declares_its_three_controls_not_its_readout() {
    use crate::components::zoom_control::ZOOM_CONTROL_WIDTH_PX;

    // Built directly rather than through `spec`: that helper reads presence off
    // a non-empty label, and the zoom slot is the one `specs()` marks present
    // unconditionally — it has no label, which is the whole point here.
    let zoom = SlotSpec {
        slot: StatusSlot::Zoom,
        present: true,
        label: "",
    };
    let (items, slots) = build_items(&[zoom]);
    assert_eq!(slots, vec![StatusSlot::Zoom]);
    assert!(
        items[0].width_px >= ZOOM_CONTROL_WIDTH_PX,
        "the zoom slot declared {} px, less than the {ZOOM_CONTROL_WIDTH_PX} px \
         the control paints",
        items[0].width_px
    );

    // And it is not a label estimate: the readout text would come to well under
    // half of this, which is the arithmetic that produced the defect.
    let readout_estimate =
        crate::responsive::estimate_label_px("1000%", crate::tokens::typography::FONT_SIZE_XS);
    assert!(
        items[0].width_px > readout_estimate * 2.0,
        "the zoom slot is still being declared like a text label"
    );
}

/// The whole bar must fit a phone-width window once the engine has had its say —
/// the acceptance line T7.1 states ("status bar legible at 320 px"), checked
/// against the declared widths rather than on a screen.
///
/// This is the assertion the under-declared zoom slot would have failed, and it
/// fails for *any* retained item that declares less than it paints.
#[test]
fn the_resolved_bar_fits_a_phone_width_window() {
    use crate::tokens::layout::RIBBON_OVERFLOW_BUTTON_PX;

    let (items, slots) = build_items(&full_bar());
    let fit = resolve_status_fit(&items, 320.0, 0);
    let painted: f32 = items
        .iter()
        .zip(&fit.shown)
        .filter(|(_, &s)| s)
        .map(|(i, _)| i.width_px)
        .sum::<f32>()
        + if fit.overflow {
            RIBBON_OVERFLOW_BUTTON_PX
        } else {
            0.0
        };
    assert!(
        painted <= 320.0,
        "the resolved bar declares {painted} px at a 320 px window — it will \
         overflow, and the overflow trigger is the thing pushed off the edge"
    );
    // The retention set is what is left, and it is still there.
    assert!(is_shown(&slots, &fit.shown, StatusSlot::Page));
    assert!(is_shown(&slots, &fit.shown, StatusSlot::Zoom));
}
