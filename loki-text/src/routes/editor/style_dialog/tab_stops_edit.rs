// SPDX-License-Identifier: Apache-2.0

//! Editing the tab-stop list.
//!
//! Split from `tab_stops` for the file ceiling. [`edited_stops`] is the pure
//! core and carries the reasoning for why an edit starts from the *shown* list.

use dioxus::prelude::*;
use loki_doc_model::style::props::tab_stop::TabStop;

use super::draft::parse_points;
use super::fields::DraftSignal;

/// Applies `edit` to the list the table is showing, sorted back into order.
///
/// **`shown` is the base, not the style's local list.** A style that inherits
/// its stops has `tab_stops == None` while the table displays the parent's set,
/// so starting from the local list would start from nothing: deleting one of
/// three inherited stops would write an empty list and lose all three, and
/// adding one would replace the inherited set with a single stop. Materialising
/// the shown list is what confines the edit to this style — which is what the
/// note under the table promises.
#[must_use]
pub(super) fn edited_stops(shown: &[TabStop], edit: impl Fn(&mut Vec<TabStop>)) -> Vec<TabStop> {
    let mut list = shown.to_vec();
    edit(&mut list);
    // Stops are positional, and the layout engine walks them in order.
    list.sort_by(|a, b| {
        a.position
            .value()
            .partial_cmp(&b.position.value())
            .unwrap_or(std::cmp::Ordering::Equal)
    });
    list
}

/// Writes the edited list onto the draft. `shown` is the resolved list the
/// table is rendering — see [`edited_stops`].
pub(super) fn edit_stops(
    mut draft: DraftSignal,
    shown: &[TabStop],
    edit: impl Fn(&mut Vec<TabStop>),
) {
    let mut next = draft.read().clone();
    if let Some(d) = next.as_mut() {
        d.style.para_props.tab_stops = Some(edited_stops(shown, edit));
    }
    draft.set(next);
}

/// Adds the stop in the "new stop at" buffer, then clears the buffer.
pub(super) fn add_stop(mut draft: DraftSignal, shown: &[TabStop]) {
    let buffer = draft
        .read()
        .as_ref()
        .map(|d| d.buffers.new_tab_stop.clone())
        .unwrap_or_default();
    let Ok(Some(position)) = parse_points(&buffer) else {
        return;
    };
    edit_stops(draft, shown, move |list| {
        // A second stop at the same position is unreachable — the first one
        // consumes the tab — so replace rather than accumulate.
        if let Some(existing) = list
            .iter_mut()
            .find(|s| (s.position.value() - position.value()).abs() < f64::EPSILON)
        {
            existing.position = position;
        } else {
            list.push(TabStop::left(position));
        }
    });
    let mut next = draft.read().clone();
    if let Some(d) = next.as_mut() {
        d.buffers.new_tab_stop = String::new();
    }
    draft.set(next);
}

#[cfg(test)]
#[path = "tab_stops_tests.rs"]
mod tests;
