// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 AppThere Loki contributors

//! The status bar's item list and its priority order (Spec 08 T7.1).
//!
//! Split from [`super::status_bar`] so the bar's *policy* — which items exist,
//! what they are worth, and how wide each declares itself — is readable and
//! testable apart from the rsx that paints them.
//!
//! # The priority order, and why it is this one
//!
//! T7.1 fixes the retention set: **the page indicator and the zoom control stay
//! at every width.** They answer "where am I" and "how big is this", which are
//! the two questions the bar exists for, and they are the two a reader cannot
//! recover from the document itself.
//!
//! Everything else is ordered by how badly its absence would be missed:
//!
//! | item | priority | why |
//! | --- | --- | --- |
//! | notice chip | 5 | a warning the user has to be able to get back to |
//! | status note | 4 | transient ("Document saved") — brief, but it is feedback for an action just taken |
//! | view mode | 3 | a control, not a readout; the others below are readouts |
//! | collaborators | 2 | matters only in a shared document, where it matters a lot |
//! | word count | 1 | a readout, recoverable from the Review tab |
//! | language | 1 | a readout, recoverable from the same place |
//!
//! Word count and language tie deliberately: nothing distinguishes them, and a
//! made-up ordering would be a decision nobody could check. The tie breaks by
//! layout order, which is the engine's documented behaviour.

use crate::responsive::{estimate_label_px, StatusItem};
use crate::tokens::spacing::{SPACE_2, SPACE_4, TOUCH_MIN};
use crate::tokens::typography::FONT_SIZE_XS;

/// Which status-bar item a [`StatusItem`] came from.
///
/// The engine returns visibility by **index**, so the bar has to map indices
/// back to items. An enum rather than a bare index: the two lists are built and
/// read in different functions, and a positional convention shared across a
/// module boundary is the kind that survives until someone inserts an item.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum StatusSlot {
    Page,
    WordCount,
    Notice,
    StatusNote,
    Language,
    ViewMode,
    Zoom,
    Collaborators,
}

/// One candidate item: its slot, whether the caller supplied it at all, and the
/// label whose width it declares.
pub(super) struct SlotSpec<'a> {
    pub slot: StatusSlot,
    /// `false` when the app passed no label for it — an absent item takes no
    /// width and can neither be shown nor dropped.
    pub present: bool,
    pub label: &'a str,
}

/// The declared width of a plain text label, plus the bar's inter-item gap.
///
/// The gap is charged to the item because it is the space the item costs the
/// bar: an item that leaves when it is dropped takes its gap with it.
fn label_width(label: &str) -> f32 {
    estimate_label_px(label, FONT_SIZE_XS) + SPACE_4
}

/// The declared width of a chip control — a label inside a padded pill inside a
/// [`TOUCH_MIN`]-wide hit area.
fn chip_width(label: &str) -> f32 {
    let visual = estimate_label_px(label, FONT_SIZE_XS) + 2.0 * SPACE_2;
    visual.max(TOUCH_MIN) + SPACE_4
}

/// The priority for a slot; see the module table. Retained slots return `0` —
/// unused, because [`StatusItem::retained`] takes them out of the drop order
/// before priority is ever read.
fn priority(slot: StatusSlot) -> u8 {
    match slot {
        StatusSlot::Page | StatusSlot::Zoom => 0,
        StatusSlot::Notice => 5,
        StatusSlot::StatusNote => 4,
        StatusSlot::ViewMode => 3,
        StatusSlot::Collaborators => 2,
        StatusSlot::WordCount | StatusSlot::Language => 1,
    }
}

/// The minimum retention set (T7.1): these two are never offered to the drop
/// order.
fn is_retained(slot: StatusSlot) -> bool {
    matches!(slot, StatusSlot::Page | StatusSlot::Zoom)
}

/// `true` for slots that render as a padded chip rather than bare text.
fn is_chip(slot: StatusSlot) -> bool {
    matches!(
        slot,
        StatusSlot::Notice | StatusSlot::StatusNote | StatusSlot::ViewMode
    )
}

/// Builds the engine's item list from the present slots, in layout order.
///
/// Absent slots are **omitted rather than zero-width**: a zero-width item would
/// still occupy a position in the returned `shown` vector, and every consumer
/// would then have to remember that "shown" does not imply "renders". The
/// returned `Vec<StatusSlot>` is the index map back.
pub(super) fn build_items(specs: &[SlotSpec<'_>]) -> (Vec<StatusItem>, Vec<StatusSlot>) {
    let mut items = Vec::new();
    let mut slots = Vec::new();
    for spec in specs.iter().filter(|s| s.present) {
        items.push(StatusItem {
            priority: priority(spec.slot),
            width_px: match spec.slot {
                StatusSlot::Zoom => {
                    crate::components::zoom_control::ZOOM_CONTROL_WIDTH_PX + SPACE_4
                }
                s if is_chip(s) => chip_width(spec.label),
                _ => label_width(spec.label),
            },
            retained: is_retained(spec.slot),
        });
        slots.push(spec.slot);
    }
    (items, slots)
}

/// Whether `slot` is visible in the bar, given the engine's answer and the index
/// map from [`build_items`].
///
/// A slot the caller did not supply is absent from `slots` and answers `false` —
/// which is the same answer "dropped" gives, and correctly so: neither renders
/// in the bar. The overflow content asks a different question (see
/// [`dropped_slots`]) and is what separates them.
#[must_use]
pub(super) fn is_shown(slots: &[StatusSlot], shown: &[bool], slot: StatusSlot) -> bool {
    slots
        .iter()
        .position(|s| *s == slot)
        .and_then(|i| shown.get(i).copied())
        .unwrap_or(false)
}

/// The slots that were dropped into the overflow, in layout order.
///
/// Distinct from "not shown": an absent slot is not dropped, it does not exist,
/// and listing it in the popover would offer the user a control the app never
/// supplied.
#[must_use]
pub(super) fn dropped_slots(slots: &[StatusSlot], shown: &[bool]) -> Vec<StatusSlot> {
    slots
        .iter()
        .zip(shown)
        .filter(|(_, &visible)| !visible)
        .map(|(slot, _)| *slot)
        .collect()
}

#[cfg(test)]
#[path = "status_bar_items_tests.rs"]
mod tests;

// ── Bridging the props to the engine and to the overflow ──────────────────────

use super::overflow::StatusOverflowRow;
use super::props::AtStatusBarProps;

/// Every candidate item, in layout order, from the bar's props.
///
/// One list, read twice — once to build the engine's items and once to build the
/// overflow rows. A second enumeration of "which items exist" is the thing that
/// drifts: the bar would drop an item the menu never offered, and it would
/// simply be gone.
pub(super) fn specs(props: &AtStatusBarProps) -> Vec<SlotSpec<'_>> {
    vec![
        SlotSpec {
            slot: StatusSlot::Page,
            present: !props.page_label.is_empty(),
            label: &props.page_label,
        },
        SlotSpec {
            slot: StatusSlot::WordCount,
            present: !props.word_count_label.is_empty(),
            label: &props.word_count_label,
        },
        SlotSpec {
            slot: StatusSlot::Notice,
            present: !props.notice_label.is_empty(),
            label: &props.notice_label,
        },
        SlotSpec {
            slot: StatusSlot::StatusNote,
            present: !props.status_note_label.is_empty(),
            label: &props.status_note_label,
        },
        SlotSpec {
            slot: StatusSlot::Language,
            present: !props.language_label.is_empty(),
            label: &props.language_label,
        },
        SlotSpec {
            slot: StatusSlot::ViewMode,
            present: !props.view_mode_label.is_empty(),
            label: &props.view_mode_label,
        },
        // The zoom control is three controls, not a piece of text, so it is the
        // one slot whose width comes from a constant rather than a label — see
        // `ZOOM_CONTROL_WIDTH_PX`, which lives next to the rsx that produces it.
        // It is retained, so this declaration never decides whether *it* shows;
        // it decides how much room is left for everything else, which is why
        // under-declaring it overflowed the bar rather than hiding it.
        SlotSpec {
            slot: StatusSlot::Zoom,
            present: true,
            label: "",
        },
        SlotSpec {
            slot: StatusSlot::Collaborators,
            present: props.collaborator_count > 0,
            label: &props.collaborator_label,
        },
    ]
}

/// The overflow menu's rows for the `dropped` slots, in layout order.
///
/// Readouts carry no action; the two controls carry theirs, so an item moved
/// into the menu is still the control it was in the bar rather than a label of
/// one.
pub(super) fn overflow_rows(
    props: &AtStatusBarProps,
    dropped: &[StatusSlot],
) -> Vec<StatusOverflowRow> {
    dropped
        .iter()
        .filter_map(|slot| {
            let (label, aria, on_click) = match slot {
                StatusSlot::WordCount => (
                    props.word_count_label.clone(),
                    props.word_count_label.clone(),
                    None,
                ),
                StatusSlot::Language => (
                    props.language_label.clone(),
                    props.language_label.clone(),
                    None,
                ),
                StatusSlot::Collaborators => (
                    props.collaborator_label.clone(),
                    props.collaborator_label.clone(),
                    None,
                ),
                StatusSlot::Notice => (
                    props.notice_label.clone(),
                    props.notice_aria_label.clone(),
                    Some(props.on_notice_click),
                ),
                StatusSlot::StatusNote => (
                    props.status_note_label.clone(),
                    props.status_note_label.clone(),
                    Some(props.on_status_note_click),
                ),
                StatusSlot::ViewMode => (
                    props.view_mode_label.clone(),
                    props.view_mode_aria_label.clone(),
                    Some(props.on_view_mode_click),
                ),
                // Retained — unreachable, because these are never in the drop
                // order. Filtered rather than `unreachable!()`: a panic in a
                // render is a worse answer to a future edit than a missing row.
                StatusSlot::Page | StatusSlot::Zoom => return None,
            };
            Some(StatusOverflowRow {
                label,
                aria_label: aria,
                on_click,
            })
        })
        .collect()
}
