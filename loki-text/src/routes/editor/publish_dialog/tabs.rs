// SPDX-License-Identifier: Apache-2.0

//! The publish dialog's tab set and its export options.

use loki_i18n::fl;

/// The five tabs of the EPUB publish dialog, in strip order.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum PublishTab {
    /// Chapter splitting, navigation, and the cover.
    Content,
    /// A read-through of what will be written into the package.
    Metadata,
    /// The accessibility claims the package will carry.
    Accessibility,
    /// The faces the document uses and whether they can be embedded.
    Fonts,
    /// File name, location, and the writer's options.
    Output,
}

impl PublishTab {
    /// Every tab, in strip order.
    pub const ALL: [PublishTab; 5] = [
        PublishTab::Content,
        PublishTab::Metadata,
        PublishTab::Accessibility,
        PublishTab::Fonts,
        PublishTab::Output,
    ];

    /// How many tabs keep an inline slot at Medium.
    pub const INLINE_AT_MEDIUM: usize = 3;

    /// The tab's position in the strip.
    #[must_use]
    pub fn index(self) -> usize {
        PublishTab::ALL.iter().position(|t| *t == self).unwrap_or(0)
    }

    /// The tab at `index`, saturating at the last tab.
    #[must_use]
    pub fn from_index(index: usize) -> Self {
        PublishTab::ALL
            .get(index)
            .copied()
            .unwrap_or(PublishTab::Output)
    }

    /// The localized strip label.
    #[must_use]
    pub fn label(self) -> String {
        match self {
            PublishTab::Content => fl!("publish-dialog-tab-content"),
            PublishTab::Metadata => fl!("publish-dialog-tab-metadata"),
            PublishTab::Accessibility => fl!("publish-dialog-tab-accessibility"),
            PublishTab::Fonts => fl!("publish-dialog-tab-fonts"),
            PublishTab::Output => fl!("publish-dialog-tab-output"),
        }
    }

    /// Every label, in strip order.
    #[must_use]
    pub fn labels() -> Vec<String> {
        PublishTab::ALL.iter().map(|t| t.label()).collect()
    }
}

/// How deep the generated table of contents goes.
///
/// The one content option the exporter honours today: `TocEntry` carries a
/// level, so filtering the navigation document by depth is a real setting
/// rather than a recorded intention.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) struct TocDepth(pub u8);

impl Default for TocDepth {
    fn default() -> Self {
        TocDepth(3)
    }
}

impl TocDepth {
    /// The depths offered, in display order.
    pub const CHOICES: [u8; 6] = [1, 2, 3, 4, 5, 6];

    /// The localized label for a depth.
    #[must_use]
    pub fn label(depth: u8) -> String {
        fl!("publish-dialog-toc-depth-level", depth = i64::from(depth))
    }

    /// This depth's position among the choices.
    #[must_use]
    pub fn index(self) -> usize {
        TocDepth::CHOICES
            .iter()
            .position(|d| *d == self.0)
            .unwrap_or(2)
    }

    /// The depth at `index`, saturating at the deepest.
    #[must_use]
    pub fn from_index(index: usize) -> Self {
        TocDepth(TocDepth::CHOICES.get(index).copied().unwrap_or(6))
    }
}

/// The export options this dialog collects.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub(super) struct PublishOptions {
    /// How deep the navigation document goes.
    pub toc_depth: TocDepth,
    /// Where the file will be written, as a display path.
    pub file_name: String,
}

#[cfg(test)]
#[path = "tabs_tests.rs"]
mod tests;
