// SPDX-License-Identifier: Apache-2.0

//! The pure page-geometry preset transform (Spec 05 M6 page family) — split from
//! [`super::page_form`]'s component so the applier stays under the 300-line
//! ceiling and the transform stays unit-testable without a Dioxus scope.

use loki_doc_model::layout::page::{PageLayout, PageOrientation, PageSize, SectionColumns};
use loki_doc_model::layout::paper_catalog::Paper;
use loki_doc_model::loki_primitives::units::Points;

/// A page-geometry preset the form can apply to a page style.
#[derive(Clone, Copy, PartialEq)]
pub(super) enum PagePreset {
    Portrait,
    Landscape,
    /// Set the page to a catalogued paper, keeping the current orientation.
    ///
    /// One variant over the whole catalogue rather than a variant per paper:
    /// the two that existed (`SizeA4`/`SizeLetter`) each carried their own
    /// dimension literals *and* their own "is this that paper" comparison, which
    /// is why only two sizes were reachable.
    Size(&'static Paper),
    MarginsNormal,
    MarginsNarrow,
    MarginsWide,
    Columns(u8),
    /// Step the column count by ±1, clamped to `1..=MAX_COLUMNS`. The three
    /// `Columns(n)` presets mirror the Layout ribbon's buttons; this is what
    /// reaches the counts the ribbon has no button for — `SectionColumns::count`
    /// has always been a `u8`, and both formats and the layout engine carry
    /// arbitrary counts, so 4-up was a gap in the panel alone.
    ColumnCountDelta(i8),
    /// Flip `SectionColumns::separator` — the vertical rule between columns.
    /// Modelled, written by both exporters, painted by the layout engine, and
    /// until now settable from no UI in the suite.
    ToggleSeparator,
}

/// The default inter-column gap when the form first adds columns (0.5 in),
/// matching the Layout ribbon.
const DEFAULT_COL_GAP_PT: f64 = 36.0;

/// The most columns the form will step to. Not a model limit — `count` is a
/// `u8` — but a page narrower than its columns lays out nothing readable, and
/// LibreOffice stops offering presets around here too.
pub(super) const MAX_COLUMNS: u8 = 12;

/// The effective column count of a layout (absent columns = a single column).
#[must_use]
pub(super) fn column_count(layout: &PageLayout) -> u8 {
    layout.columns.as_ref().map_or(1, |c| c.count).max(1)
}

/// Returns `current` with `preset` applied — the pure page-geometry transform.
/// Orientation and size preserve the other axis (choosing A4 while landscape
/// stays landscape); margins keep header/footer/gutter; columns keep the gap.
#[must_use]
pub(super) fn apply_preset(current: &PageLayout, preset: PagePreset) -> PageLayout {
    let mut l = current.clone();
    let is_landscape = l.page_size.width.value() > l.page_size.height.value();
    match preset {
        PagePreset::Portrait | PagePreset::Landscape => {
            let want = preset == PagePreset::Landscape;
            l.orientation = if want {
                PageOrientation::Landscape
            } else {
                PageOrientation::Portrait
            };
            if is_landscape != want {
                let (w, h) = (l.page_size.width, l.page_size.height);
                l.page_size = PageSize {
                    width: h,
                    height: w,
                };
            }
        }
        PagePreset::Size(paper) => {
            l.page_size = paper.oriented_like(&l.page_size);
        }
        PagePreset::MarginsNormal | PagePreset::MarginsNarrow | PagePreset::MarginsWide => {
            let (tb, lr) = match preset {
                PagePreset::MarginsNarrow => (36.0, 36.0),
                PagePreset::MarginsWide => (72.0, 144.0),
                _ => (72.0, 72.0),
            };
            l.margins.top = Points::new(tb);
            l.margins.bottom = Points::new(tb);
            l.margins.left = Points::new(lr);
            l.margins.right = Points::new(lr);
        }
        PagePreset::ColumnCountDelta(d) => {
            // Saturating `u8` arithmetic rather than a widened signed add and a
            // cast back: the count *is* a `u8`, so staying in it keeps both
            // clamps total without a truncation to suppress.
            let now = column_count(&l);
            let next = if d < 0 {
                now.saturating_sub(d.unsigned_abs()).max(1)
            } else {
                now.saturating_add(d.unsigned_abs()).min(MAX_COLUMNS)
            };
            // Reuse the `Columns` arm so stepping and the presets cannot drift
            // over what "3 columns" means (gap kept, mismatched widths dropped).
            return apply_preset(current, PagePreset::Columns(next));
        }
        PagePreset::ToggleSeparator => {
            if let Some(c) = l.columns.as_mut() {
                c.separator = !c.separator;
            }
        }
        PagePreset::Columns(n) => {
            l.columns = if n <= 1 {
                None
            } else {
                let gap = l
                    .columns
                    .as_ref()
                    .map_or(Points::new(DEFAULT_COL_GAP_PT), |c| c.gap);
                // Preserve explicit per-column widths only when they still match
                // the new column count; otherwise fall back to equal columns.
                let widths = l
                    .columns
                    .as_ref()
                    .map(|c| c.widths.clone())
                    .filter(|w| w.len() == usize::from(n))
                    .unwrap_or_default();
                Some(SectionColumns {
                    count: n,
                    gap,
                    separator: l.columns.as_ref().is_some_and(|c| c.separator),
                    widths,
                })
            };
        }
    }
    l
}

/// Whether `layout` already matches `preset` (drives the active-button styling).
pub(super) fn is_active(layout: &PageLayout, preset: PagePreset) -> bool {
    let landscape = layout.page_size.width.value() > layout.page_size.height.value();
    let m = &layout.margins;
    let all = |v: f64| (m.top.value() - v).abs() < 0.5 && (m.bottom.value() - v).abs() < 0.5;
    let lr = |v: f64| (m.left.value() - v).abs() < 0.5 && (m.right.value() - v).abs() < 0.5;
    let count = layout.columns.as_ref().map_or(1, |c| c.count);
    match preset {
        PagePreset::Portrait => !landscape,
        PagePreset::Landscape => landscape,
        PagePreset::Size(paper) => paper.matches(&layout.page_size),
        PagePreset::MarginsNormal => all(72.0),
        PagePreset::MarginsNarrow => all(36.0),
        PagePreset::MarginsWide => all(72.0) && lr(144.0),
        PagePreset::Columns(n) => count == n,
        // A step is an action, not a state: it is never the "current" value.
        PagePreset::ColumnCountDelta(_) => false,
        PagePreset::ToggleSeparator => layout.columns.as_ref().is_some_and(|c| c.separator),
    }
}

#[cfg(test)]
#[path = "page_presets_tests.rs"]
mod tests;
