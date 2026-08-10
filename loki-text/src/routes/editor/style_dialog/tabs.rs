// SPDX-License-Identifier: Apache-2.0

//! The paragraph style dialog's tab set and its collapse priority.

use loki_i18n::fl;

/// The seven tabs of the paragraph style editor, in strip order.
///
/// Strip order is the LibreOffice-familiar grouping — identity, then the
/// character aspect, then the paragraph aspects. It is **not** the collapse
/// priority; see [`ParaTab::INLINE_AT_MEDIUM`].
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum ParaTab {
    /// Name, parent, next style, and the inheritance chain.
    General,
    /// The style's run-default character properties.
    Font,
    /// Indents and paragraph spacing.
    Indents,
    /// Horizontal alignment.
    Alignment,
    /// Pagination and keep-together rules.
    TextFlow,
    /// Paragraph borders and their padding.
    Borders,
    /// The tab-stop table.
    TabStops,
}

impl ParaTab {
    /// Every tab, in strip order.
    pub const ALL: [ParaTab; 7] = [
        ParaTab::General,
        ParaTab::Font,
        ParaTab::Indents,
        ParaTab::Alignment,
        ParaTab::TextFlow,
        ParaTab::Borders,
        ParaTab::TabStops,
    ];

    /// How many tabs keep an inline slot at the Medium size class.
    ///
    /// # This number is a judgement, and it is the one to tune first
    ///
    /// Three keeps General / Font / Indents — the tabs a user opens this dialog
    /// for — inline, and pushes the four situational ones into `More ▾`. The
    /// strip guarantees the *active* tab is always reachable without opening
    /// the menu ([`appthere_ui::DialogTabLayout`]), so the cost of being wrong
    /// here is an extra click on a tab someone visits often, not a hidden body.
    /// It is a prop rather than a constant in `appthere_ui` precisely so each
    /// dialog can disagree.
    pub const INLINE_AT_MEDIUM: usize = 3;

    /// The tab's position in the strip.
    #[must_use]
    pub fn index(self) -> usize {
        ParaTab::ALL
            .iter()
            .position(|t| *t == self)
            .unwrap_or_default()
    }

    /// The tab at `index`, saturating at the last tab.
    ///
    /// Saturates rather than panicking: the index arrives from the strip, whose
    /// label vector and this enum could disagree for one frame during a redraw.
    #[must_use]
    pub fn from_index(index: usize) -> Self {
        ParaTab::ALL
            .get(index)
            .copied()
            .unwrap_or(ParaTab::TabStops)
    }

    /// The localized strip label.
    #[must_use]
    pub fn label(self) -> String {
        match self {
            ParaTab::General => fl!("style-dialog-tab-general"),
            ParaTab::Font => fl!("style-dialog-tab-font"),
            ParaTab::Indents => fl!("style-dialog-tab-indents"),
            ParaTab::Alignment => fl!("style-dialog-tab-alignment"),
            ParaTab::TextFlow => fl!("style-dialog-tab-text-flow"),
            ParaTab::Borders => fl!("style-dialog-tab-borders"),
            ParaTab::TabStops => fl!("style-dialog-tab-tab-stops"),
        }
    }

    /// Every label, in strip order — what the strip renders.
    #[must_use]
    pub fn labels() -> Vec<String> {
        ParaTab::ALL.iter().map(|t| t.label()).collect()
    }

    /// Whether this tab shows the paper preview rail.
    ///
    /// General and Tab stops carry their own visualisations — the inheritance
    /// chain and the leader-dot specimen — so a second preview beside them
    /// would show the same paragraph twice and steal width from the table.
    #[must_use]
    pub fn has_preview(self) -> bool {
        !matches!(self, ParaTab::General | ParaTab::TabStops)
    }
}

#[cfg(test)]
#[path = "tabs_tests.rs"]
mod tests;
