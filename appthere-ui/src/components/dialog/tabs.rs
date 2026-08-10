// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 AppThere Loki contributors

//! How a dialog's tab strip presents itself at each size class (design notes
//! 03/04).
//!
//! A tabbed dialog exposes the **same tab set at every size class** — nothing is
//! dropped, only re-laid-out:
//!
//! | class | presentation |
//! | --- | --- |
//! | Expanded | every tab inline |
//! | Medium | the first `inline` tabs, the rest behind `More ▾ <n>` |
//! | Compact | one section picker row — `2 / 7 · Font ▾` |
//!
//! The active tab is **always reachable without opening a menu**: if it would
//! fall in the overflow, it takes the last inline slot. Without that rule the
//! strip can show a `More ▾` menu next to a body whose owning tab is nowhere on
//! screen, which reads as a rendering bug rather than a collapse.
//!
//! Which tabs earn the inline slots is a per-dialog editorial decision, so the
//! count is a parameter rather than a constant here
//! ([`tokens::DIALOG_TABS_INLINE_MEDIUM`] is only the default).

use crate::responsive::Breakpoint;
use crate::tokens;

/// The presentation a tab strip adopts for a size class.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum DialogTabLayout {
    /// Every tab is inline (Expanded).
    Inline,
    /// `visible` tabs inline, `overflow` behind a `More ▾` menu (Medium).
    Overflow {
        /// How many tabs keep an inline slot.
        visible: usize,
        /// How many tabs sit in the menu — the count shown beside `More ▾`.
        overflow: usize,
    },
    /// One picker row standing in for the whole strip (Compact).
    Picker,
}

impl DialogTabLayout {
    /// The layout for `total` tabs at `bp`, keeping `inline` of them inline at
    /// Medium.
    ///
    /// Degenerate inputs resolve to [`DialogTabLayout::Inline`] rather than an
    /// empty strip: a strip that would show *no* tabs and a `More ▾ 7` menu is
    /// strictly worse than an overflowing row of labels.
    #[must_use]
    pub fn for_breakpoint(bp: Breakpoint, total: usize, inline: usize) -> Self {
        match bp {
            Breakpoint::Compact => DialogTabLayout::Picker,
            Breakpoint::Expanded => DialogTabLayout::Inline,
            Breakpoint::Medium => {
                // Collapsing to leave one tab in the menu costs a click and a
                // menu to save one label — never worth it.
                if inline == 0 || total <= inline + 1 {
                    DialogTabLayout::Inline
                } else {
                    DialogTabLayout::Overflow {
                        visible: inline,
                        overflow: total - inline,
                    }
                }
            }
        }
    }

    /// The layout at Medium using the default inline count.
    #[must_use]
    pub fn for_breakpoint_default(bp: Breakpoint, total: usize) -> Self {
        Self::for_breakpoint(bp, total, tokens::DIALOG_TABS_INLINE_MEDIUM)
    }

    /// The indices that render inline, in strip order, given the active tab.
    ///
    /// The active tab always appears: when it would overflow it displaces the
    /// last inline slot, so the strip never shows a body whose tab is hidden.
    #[must_use]
    pub fn inline_indices(self, total: usize, active: usize) -> Vec<usize> {
        match self {
            DialogTabLayout::Inline => (0..total).collect(),
            DialogTabLayout::Picker => Vec::new(),
            DialogTabLayout::Overflow { visible, .. } => {
                let mut shown: Vec<usize> = (0..visible.min(total)).collect();
                if active < total && !shown.contains(&active) {
                    // Replace the last slot rather than appending, so the strip
                    // width stays constant as the user moves between tabs.
                    if let Some(last) = shown.last_mut() {
                        *last = active;
                    }
                }
                shown
            }
        }
    }

    /// The indices behind the `More ▾` menu, in strip order, given the active
    /// tab. Always the exact complement of [`Self::inline_indices`].
    #[must_use]
    pub fn overflow_indices(self, total: usize, active: usize) -> Vec<usize> {
        let shown = self.inline_indices(total, active);
        (0..total).filter(|i| !shown.contains(i)).collect()
    }
}

#[cfg(test)]
#[path = "tabs_tests.rs"]
mod tests;
