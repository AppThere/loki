// SPDX-License-Identifier: Apache-2.0

//! Tests for [`super::TableSpec`] and [`super::build_table`].

use super::*;

fn spec(rows: usize, cols: usize) -> TableSpec {
    TableSpec {
        rows,
        cols,
        ..TableSpec::default()
    }
}

/// Design note 24: header row is on by default, because an unheaded table is
/// inaccessible and the export pipeline cannot guess one later.
#[test]
fn a_header_row_is_on_by_default() {
    assert!(TableSpec::default().header_row);
    assert_eq!(TableSpec::default().rows, 4);
    assert_eq!(TableSpec::default().cols, 3);
}

/// The grid and steppers count rows the user *sees*, so a 4-row table with a
/// header is one header plus three body rows — not four body rows plus a fifth.
#[test]
fn the_header_row_is_promoted_out_of_the_body_not_added_to_it() {
    let table = build_table(&spec(4, 3));
    assert_eq!(table.head.rows.len(), 1);
    let body_rows: usize = table.bodies.iter().map(|b| b.body_rows.len()).sum();
    assert_eq!(body_rows, 3);
    assert_eq!(body_rows + table.head.rows.len(), 4, "four rows on screen");
}

#[test]
fn without_a_header_every_row_stays_in_the_body() {
    let s = TableSpec {
        header_row: false,
        ..spec(4, 3)
    };
    let table = build_table(&s);
    assert!(table.head.rows.is_empty());
    let body_rows: usize = table.bodies.iter().map(|b| b.body_rows.len()).sum();
    assert_eq!(body_rows, 4);
    assert_eq!(s.body_rows(), 4);
}

/// Column count is not disturbed by the header promotion.
#[test]
fn every_row_has_the_requested_column_count() {
    let table = build_table(&spec(4, 5));
    for row in &table.head.rows {
        assert_eq!(row.cells.len(), 5);
    }
    for body in &table.bodies {
        for row in &body.body_rows {
            assert_eq!(row.cells.len(), 5);
        }
    }
}

/// A zero of either axis is not a smaller table but no table at all, so the
/// clamp must actually bite at the bottom as well as the top.
#[test]
fn the_size_clamp_bites_at_both_ends() {
    let low = spec(0, 0).clamped();
    assert_eq!((low.rows, low.cols), (1, 1));

    let high = spec(10_000, 10_000).clamped();
    assert_eq!((high.rows, high.cols), (MAX_ROWS, MAX_COLS));

    let ok = spec(4, 3).clamped();
    assert_eq!((ok.rows, ok.cols), (4, 3), "a legal size is untouched");
}

/// A one-row table asked for a header keeps that row as the header rather than
/// silently dropping the header the user asked for.
#[test]
fn a_single_row_table_with_a_header_has_no_body() {
    let table = build_table(&spec(1, 3));
    assert_eq!(table.head.rows.len(), 1);
    let body_rows: usize = table.bodies.iter().map(|b| b.body_rows.len()).sum();
    assert_eq!(body_rows, 0);
    assert_eq!(spec(1, 3).body_rows(), 0);
}

/// A clamped-to-one table must not underflow when the header takes its row.
#[test]
fn body_row_count_does_not_underflow_at_the_clamp_floor() {
    assert_eq!(spec(0, 0).body_rows(), 0);
}

#[test]
fn a_caption_is_written_and_an_empty_one_is_not() {
    let with = build_table(&TableSpec {
        caption: "  Tide times, week of 4 May  ".to_string(),
        ..spec(3, 2)
    });
    assert_eq!(
        with.caption.full,
        vec![Inline::Str("Tide times, week of 4 May".to_string())],
        "the caption is trimmed"
    );

    let without = build_table(&TableSpec {
        caption: "   ".to_string(),
        ..spec(3, 2)
    });
    assert!(
        without.caption.full.is_empty(),
        "whitespace is not a caption"
    );
}

/// The two width choices must reach different model values, or the control
/// changes nothing.
#[test]
fn the_width_choices_map_to_distinct_model_widths() {
    assert_eq!(
        build_table(&spec(2, 2)).width,
        Some(TableWidth::Percent(100.0))
    );
    let auto = build_table(&TableSpec {
        width: WidthChoice::Auto,
        ..spec(2, 2)
    });
    assert_eq!(auto.width, Some(TableWidth::Auto));
}

#[test]
fn width_choices_round_trip_through_their_segment_index() {
    for (i, choice) in WidthChoice::ALL.iter().enumerate() {
        assert_eq!(choice.index(), i);
        assert_eq!(WidthChoice::from_index(i), *choice);
    }
    assert_eq!(WidthChoice::from_index(99), WidthChoice::Fill, "saturates");
}

/// A chosen table style binds to the table; a blank one leaves it unstyled
/// rather than binding to an empty name no catalog entry answers to.
#[test]
fn a_table_style_binds_only_when_it_names_something() {
    let styled = build_table(&TableSpec {
        style: Some("Ruled".to_string()),
        ..spec(2, 2)
    });
    assert_eq!(styled.style_name(), Some("Ruled"));

    for blank in [None, Some(String::new()), Some("   ".to_string())] {
        let table = build_table(&TableSpec {
            style: blank.clone(),
            ..spec(2, 2)
        });
        assert_eq!(table.style_name(), None, "{blank:?} should not bind");
    }
}

/// The drag grid must be a subset of what the steppers reach, or the picker
/// could request a size the steppers then clamp away under the user.
#[test]
fn the_drag_grid_fits_inside_the_stepper_range() {
    assert!(MAX_GRID_ROWS <= MAX_ROWS);
    assert!(MAX_GRID_COLS <= MAX_COLS);
    let full_grid = spec(MAX_GRID_ROWS, MAX_GRID_COLS).clamped();
    assert_eq!(full_grid.rows, MAX_GRID_ROWS);
    assert_eq!(full_grid.cols, MAX_GRID_COLS);
}
