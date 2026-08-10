// SPDX-License-Identifier: Apache-2.0

//! The metadata dialog's tab set and the field-to-tab assignment (design note
//! 15).

use loki_i18n::fl;

use super::super::editor_metadata::MetaField;

/// The five tabs of the metadata editor, in strip order.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum MetaTab {
    /// The six fields most documents actually set.
    General,
    /// The remaining Dublin Core descriptive fields.
    DublinCore,
    /// Identifier, scheme, and the publishing identifiers.
    Identifiers,
    /// EPUB accessibility metadata.
    Accessibility,
    /// Read-only document statistics.
    Statistics,
}

impl MetaTab {
    /// Every tab, in strip order.
    pub const ALL: [MetaTab; 5] = [
        MetaTab::General,
        MetaTab::DublinCore,
        MetaTab::Identifiers,
        MetaTab::Accessibility,
        MetaTab::Statistics,
    ];

    /// How many tabs keep an inline slot at Medium.
    ///
    /// Three: General and Dublin Core are where the writing happens, and
    /// Accessibility earns the third slot over Identifiers because it is the
    /// one a publisher is legally answerable for.
    pub const INLINE_AT_MEDIUM: usize = 3;

    /// The tab's position in the strip.
    #[must_use]
    pub fn index(self) -> usize {
        MetaTab::ALL.iter().position(|t| *t == self).unwrap_or(0)
    }

    /// The tab at `index`, saturating at the last tab.
    #[must_use]
    pub fn from_index(index: usize) -> Self {
        MetaTab::ALL
            .get(index)
            .copied()
            .unwrap_or(MetaTab::Statistics)
    }

    /// The localized strip label.
    #[must_use]
    pub fn label(self) -> String {
        match self {
            MetaTab::General => fl!("meta-dialog-tab-general"),
            MetaTab::DublinCore => fl!("meta-dialog-tab-dublin-core"),
            MetaTab::Identifiers => fl!("meta-dialog-tab-identifiers"),
            MetaTab::Accessibility => fl!("meta-dialog-tab-accessibility"),
            MetaTab::Statistics => fl!("meta-dialog-tab-statistics"),
        }
    }

    /// Every label, in strip order.
    #[must_use]
    pub fn labels() -> Vec<String> {
        MetaTab::ALL.iter().map(|t| t.label()).collect()
    }

    /// The fields this tab edits, in display order.
    ///
    /// Eighteen fields is too many for one wall of inputs (design note 15), so
    /// General carries the six most documents set and the rest live behind the
    /// two descriptive tabs — in the same order the model defines.
    #[must_use]
    pub fn fields(self) -> &'static [MetaField] {
        match self {
            MetaTab::General => &[
                MetaField::Title,
                MetaField::Creator,
                MetaField::Language,
                MetaField::Description,
                MetaField::Subject,
                MetaField::Keywords,
                MetaField::Publisher,
                MetaField::Contributors,
                MetaField::Rights,
                MetaField::License,
            ],
            MetaTab::DublinCore => &[
                MetaField::DcType,
                MetaField::Source,
                MetaField::Relation,
                MetaField::Coverage,
                MetaField::Issued,
                MetaField::Citation,
            ],
            MetaTab::Identifiers => &[MetaField::Identifier, MetaField::IdentifierScheme],
            // Neither tab edits a `MetaField`: accessibility metadata has no
            // model yet, and statistics are derived.
            MetaTab::Accessibility | MetaTab::Statistics => &[],
        }
    }

    /// Whether this tab's fields span the body's full width.
    ///
    /// Description and the citation are sentences; a two-column grid would give
    /// them a measure a third of a line long.
    #[must_use]
    pub fn is_wide_field(field: MetaField) -> bool {
        matches!(
            field,
            MetaField::Title | MetaField::Description | MetaField::Citation
        )
    }
}

/// The fields an EPUB export requires, in the order the preflight reports them.
///
/// `dc:title`, `dc:creator` and `dc:language` are the EPUB 3.3 §5.4 package
/// requirements; `dc:publisher` is not required by the specification but is by
/// every store that ingests one, which is the distinction the dialog's warning
/// draws rather than eliding.
pub(super) const EPUB_REQUIRED: [MetaField; 4] = [
    MetaField::Title,
    MetaField::Creator,
    MetaField::Language,
    MetaField::Publisher,
];

/// Whether `field` is needed before this document can be published as EPUB.
#[must_use]
pub(super) fn is_epub_required(field: MetaField) -> bool {
    EPUB_REQUIRED.contains(&field)
}

/// How many required fields are still empty in `values`.
///
/// The count the tab strip carries (design note 16): empty-but-required fields
/// are dashed rather than hidden, and the strip shows the running total so the
/// user does not have to open every tab to find them.
#[must_use]
pub(super) fn missing_required(values: &[(MetaField, String)]) -> usize {
    EPUB_REQUIRED
        .iter()
        .filter(|required| {
            values
                .iter()
                .find(|(f, _)| f == *required)
                .is_none_or(|(_, v)| v.trim().is_empty())
        })
        .count()
}

#[cfg(test)]
#[path = "tabs_tests.rs"]
mod tests;
