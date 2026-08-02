// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 AppThere Loki contributors

//! The zoom menu's row table and the walk over it.

use super::{next_zoom_row, prev_zoom_row, zoom_rows, ZoomCommands, ZoomRow};
use crate::components::zoom::ZOOM_PRESETS_PERCENT;

/// An app that can do the fits but has no display density — the ordinary desktop
/// case until T5.5's platform probe lands.
fn fits_only() -> ZoomCommands {
    ZoomCommands {
        fit_width: true,
        fit_page: true,
        actual_size: false,
    }
}

/// Everything available.
fn all() -> ZoomCommands {
    ZoomCommands {
        fit_width: true,
        fit_page: true,
        actual_size: true,
    }
}

/// Every preset appears, in the stepper's order, followed by the fits.
#[test]
fn the_rows_are_the_presets_then_the_fits() {
    let rows = zoom_rows(fits_only());
    assert_eq!(rows.len(), ZOOM_PRESETS_PERCENT.len() + 2);
    for (i, row) in rows.iter().take(ZOOM_PRESETS_PERCENT.len()).enumerate() {
        assert_eq!(*row, ZoomRow::Preset(i));
        assert_eq!(row.preset_percent(), Some(ZOOM_PRESETS_PERCENT[i]));
    }
    assert_eq!(rows[ZOOM_PRESETS_PERCENT.len()], ZoomRow::FitWidth);
    assert_eq!(rows[ZOOM_PRESETS_PERCENT.len() + 1], ZoomRow::FitPage);
}

/// **Actual Size is present exactly when the platform can support it.** Both
/// polarities, because a row that is always there is a command that silently
/// does nothing, and a row that is never there is a capability nobody can reach
/// — and each looks correct if you only test the other.
#[test]
fn actual_size_appears_only_when_the_display_density_is_known() {
    assert!(!zoom_rows(fits_only()).contains(&ZoomRow::ActualSize));
    assert!(zoom_rows(all()).contains(&ZoomRow::ActualSize));
    assert_eq!(zoom_rows(all()).len(), zoom_rows(fits_only()).len() + 1);
}

/// **Each computed row follows its own capability, and an app with none of them
/// still has a usable menu.** That is the spreadsheet and presentation case:
/// they share this control and have no page geometry to fit to, so a menu
/// offering them Fit Page would be offering a command with nothing behind it.
#[test]
fn each_computed_row_follows_its_own_capability() {
    let none = zoom_rows(ZoomCommands::default());
    assert_eq!(none.len(), ZOOM_PRESETS_PERCENT.len(), "presets only");
    assert!(next_zoom_row(&none, None).is_some(), "still walkable");

    let width_only = zoom_rows(ZoomCommands {
        fit_width: true,
        ..Default::default()
    });
    assert!(width_only.contains(&ZoomRow::FitWidth));
    assert!(!width_only.contains(&ZoomRow::FitPage));

    let page_only = zoom_rows(ZoomCommands {
        fit_page: true,
        ..Default::default()
    });
    assert!(page_only.contains(&ZoomRow::FitPage));
    assert!(!page_only.contains(&ZoomRow::FitWidth));
}

/// Keys are distinct **across** variants, not just within them.
#[test]
fn every_row_has_its_own_key() {
    let rows = zoom_rows(all());
    let mut keys: Vec<String> = rows.iter().map(|r| r.key()).collect();
    let before = keys.len();
    keys.sort();
    keys.dedup();
    assert_eq!(keys.len(), before, "duplicate row key in {keys:?}");
}

/// Only preset rows name a zoom; the fits are computed by the caller from
/// measurements this module does not have.
#[test]
fn only_preset_rows_carry_a_percent() {
    assert_eq!(ZoomRow::FitWidth.preset_percent(), None);
    assert_eq!(ZoomRow::FitPage.preset_percent(), None);
    assert_eq!(ZoomRow::ActualSize.preset_percent(), None);
    assert_eq!(
        ZoomRow::Preset(ZOOM_PRESETS_PERCENT.len()).preset_percent(),
        None,
        "an index past the ladder names no zoom rather than panicking",
    );
}

/// The first press enters from the end the user expects, in both directions.
#[test]
fn the_first_press_enters_from_the_right_end() {
    let rows = zoom_rows(all());
    assert_eq!(next_zoom_row(&rows, None), Some(ZoomRow::Preset(0)));
    assert_eq!(prev_zoom_row(&rows, None), Some(ZoomRow::ActualSize));
}

/// Down walks forward and wraps at the end.
#[test]
fn down_walks_forward_and_wraps() {
    let rows = zoom_rows(fits_only());
    assert_eq!(
        next_zoom_row(&rows, Some("zoom-0")),
        Some(ZoomRow::Preset(1))
    );
    assert_eq!(
        next_zoom_row(&rows, Some("fit-page")),
        Some(ZoomRow::Preset(0)),
        "past the last row is the first",
    );
}

/// Up walks backward and wraps — the polarity of the test above (L08-045),
/// without which `prev_zoom_row` could be `next_zoom_row` and go unnoticed.
#[test]
fn up_walks_backward_and_wraps() {
    let rows = zoom_rows(fits_only());
    assert_eq!(
        prev_zoom_row(&rows, Some("fit-page")),
        Some(ZoomRow::FitWidth)
    );
    assert_eq!(
        prev_zoom_row(&rows, Some("zoom-0")),
        Some(ZoomRow::FitPage),
        "before the first row is the last",
    );
}

/// **The case the pointer creates.** A hover key survives the menu reopening
/// with a different row set — Actual Size arriving when a display is plugged in
/// — so a key naming no row must land at an end rather than stranding the
/// keyboard.
#[test]
fn a_stale_key_does_not_strand_the_keyboard() {
    let rows = zoom_rows(fits_only());
    assert_eq!(
        next_zoom_row(&rows, Some("actual-size")),
        Some(ZoomRow::Preset(0)),
    );
    assert_eq!(
        prev_zoom_row(&rows, Some("actual-size")),
        Some(ZoomRow::FitPage),
    );
}

/// Down and Up are inverses at every row, including across the wrap.
#[test]
fn down_then_up_returns_to_the_same_row() {
    let rows = zoom_rows(all());
    for row in &rows {
        let down = next_zoom_row(&rows, Some(&row.key())).expect("non-empty");
        assert_eq!(prev_zoom_row(&rows, Some(&down.key())), Some(*row));
    }
}

/// The menu always has rows, so the walk is total in practice — what makes the
/// `Option` at the call site a case that cannot arise rather than one nobody
/// has thought about.
#[test]
fn the_walk_always_finds_a_row() {
    for commands in [ZoomCommands::default(), fits_only(), all()] {
        let rows = zoom_rows(commands);
        assert!(!rows.is_empty());
        assert!(next_zoom_row(&rows, None).is_some());
        assert!(prev_zoom_row(&rows, None).is_some());
    }
}
