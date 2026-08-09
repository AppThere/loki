// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 AppThere Loki contributors

//! The page-selection decision, and the two ways this wiring can be present and
//! still not work.
//!
//! What these tests cannot do is check the *bound* — that is
//! `plan_capability`'s, and re-asserting it here would be the second derivation
//! the module exists to avoid. What they hold is the wiring: which page is asked
//! about, and that "no pages" does not silently become "clamp to the floor".

use appthere_canvas::residency::{BudgetInputs, TextureBudget};

use super::{capability_limit_permille, largest_page};

/// A budget small enough that the bound is not simply the top of the range,
/// so a test can tell a real answer from a saturated one.
fn small_budget() -> TextureBudget {
    TextureBudget::derive(BudgetInputs {
        available_ram_bytes: Some(2 * 1024 * 1024 * 1024),
        total_ram_bytes: Some(2 * 1024 * 1024 * 1024),
        gpu_paint_path: Some(true),
        user_override_bytes: None,
        diagnostic_ceiling_bytes: None,
    })
}

const LETTER: (f64, f64) = (612.0, 792.0);
const A2: (f64, f64) = (1191.0, 1684.0);

/// The largest page wins regardless of where it sits in the document — the
/// property that makes the limit stable while the reader scrolls.
#[test]
fn the_largest_page_is_chosen_wherever_it_appears() {
    let first = largest_page(&[A2, LETTER, LETTER]).expect("pages");
    let middle = largest_page(&[LETTER, A2, LETTER]).expect("pages");
    let last = largest_page(&[LETTER, LETTER, A2]).expect("pages");
    assert_eq!(first, middle);
    assert_eq!(middle, last);
    assert_eq!(first, appthere_canvas::residency::PageBox::new(A2.0, A2.1));
}

/// **Area, not width or height.** A page can be the widest and not the largest,
/// and residency is paid per texel — so a bound picked by either axis alone
/// would be chosen against the wrong quantity (evidence rule 3: an instrument
/// reporting on an adjacent one).
#[test]
fn the_choice_is_by_area_not_by_either_axis() {
    // Wide and short vs. narrow and tall: the second has the greater area.
    let wide_short = (1400.0, 400.0); // 560 000
    let narrow_tall = (700.0, 1000.0); // 700 000
    assert_eq!(
        largest_page(&[wide_short, narrow_tall]),
        Some(appthere_canvas::residency::PageBox::new(700.0, 1000.0)),
        "the widest page is not the largest",
    );
}

/// **A document with no resolved pages must not be capped.** The failure this
/// pins is a zoom-out on open: were `None` to become `Some(floor)`, every
/// document would clamp to the bottom of the range for the frame before its
/// layout lands, on no evidence about the device at all.
#[test]
fn a_document_with_no_pages_is_not_capped() {
    assert_eq!(largest_page(&[]), None);
    assert_eq!(capability_limit_permille(&[], 2.0, small_budget()), None);
}

/// Degenerate page sizes are not pages. A zero-area box would win no comparison
/// but would satisfy a naive `first()`, and asking the planner about it is
/// asking a question with no answer.
#[test]
fn zero_sized_pages_are_ignored() {
    assert_eq!(largest_page(&[(0.0, 0.0), (0.0, 792.0)]), None);
    assert_eq!(
        largest_page(&[(0.0, 0.0), LETTER]),
        Some(appthere_canvas::residency::PageBox::new(LETTER.0, LETTER.1)),
    );
}

/// **The polarity (L08-045).** Every assertion above passes for a function that
/// always returns `None`, which would silently disable the clamp — the exact
/// state Phase 2 shipped. So pin that a real document produces a real bound,
/// inside the range the control offers.
#[test]
fn a_real_document_produces_a_bound_in_range() {
    let limit = capability_limit_permille(&[LETTER], 2.0, small_budget())
        .expect("a document with a page has a bound");
    assert!(
        (250..=4000).contains(&limit),
        "bound {limit} outside the offered zoom range",
    );
}

/// **A bigger page cannot afford a higher zoom.** The direction is the whole
/// point of choosing by area, and it is the assertion that fails if
/// `largest_page` is quietly changed to `min_by`.
#[test]
fn a_larger_page_never_permits_a_higher_zoom() {
    let budget = small_budget();
    let letter = capability_limit_permille(&[LETTER], 2.0, budget).expect("letter");
    let a2 = capability_limit_permille(&[A2], 2.0, budget).expect("a2");
    assert!(
        a2 <= letter,
        "A2 ({a2}) must not out-zoom Letter ({letter})",
    );

    // And the mixed document is held to the larger page, not the first one.
    let mixed = capability_limit_permille(&[LETTER, A2], 2.0, budget).expect("mixed");
    assert_eq!(mixed, a2, "a mixed document takes the A2 limit");
}
