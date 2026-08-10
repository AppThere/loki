// SPDX-License-Identifier: Apache-2.0

//! The span dialog's tab set, and the four-level provenance it reports.

use appthere_ui::AtProvenanceKind;
use loki_i18n::fl;

use super::marks::SpanMarks;

/// The five tabs of the character-formatting dialog, in strip order.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum SpanTab {
    /// Family, size, weight, colour, letter spacing.
    Font,
    /// Underline, strikethrough, case.
    Effects,
    /// Vertical position and kerning.
    Position,
    /// Highlight colour and how it exports.
    Highlight,
    /// The run's language and direction.
    Language,
}

impl SpanTab {
    /// Every tab, in strip order.
    pub const ALL: [SpanTab; 5] = [
        SpanTab::Font,
        SpanTab::Effects,
        SpanTab::Position,
        SpanTab::Highlight,
        SpanTab::Language,
    ];

    /// How many tabs keep an inline slot at Medium.
    ///
    /// Font, Effects and Position: the three a user reaches for on a selection.
    /// Highlight and Language are deliberate, occasional acts.
    pub const INLINE_AT_MEDIUM: usize = 3;

    /// The tab's position in the strip.
    #[must_use]
    pub fn index(self) -> usize {
        SpanTab::ALL.iter().position(|t| *t == self).unwrap_or(0)
    }

    /// The tab at `index`, saturating at the last tab.
    #[must_use]
    pub fn from_index(index: usize) -> Self {
        SpanTab::ALL
            .get(index)
            .copied()
            .unwrap_or(SpanTab::Language)
    }

    /// The localized strip label.
    #[must_use]
    pub fn label(self) -> String {
        match self {
            SpanTab::Font => fl!("span-dialog-tab-font"),
            SpanTab::Effects => fl!("span-dialog-tab-effects"),
            SpanTab::Position => fl!("span-dialog-tab-position"),
            SpanTab::Highlight => fl!("span-dialog-tab-highlight"),
            SpanTab::Language => fl!("span-dialog-tab-language"),
        }
    }

    /// Every label, in strip order.
    #[must_use]
    pub fn labels() -> Vec<String> {
        SpanTab::ALL.iter().map(|t| t.label()).collect()
    }
}

/// Where a span property's value resolves from.
///
/// # Four levels, not three
///
/// Direct formatting → character style → paragraph style → document default
/// (design note 09). The first is the only one "Clear direct formatting"
/// removes, so it is the only one that gets a Reset. The three below it are
/// reported but not editable from here — changing them is what the paragraph
/// and character style editors are for.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum SpanLevel {
    /// A mark on this run.
    Direct,
    /// The run's character style.
    CharacterStyle,
    /// The paragraph's style.
    ParagraphStyle,
    /// The document default.
    Document,
}

impl SpanLevel {
    /// The shared component's kind for this level.
    ///
    /// Direct formatting maps onto [`AtProvenanceKind::Local`] — the level the
    /// Clear button removes — and the three inherited levels map onto the
    /// inherited kinds so the line reads `↳ source · value` for all of them.
    #[must_use]
    pub fn kind(self) -> AtProvenanceKind {
        match self {
            SpanLevel::Direct => AtProvenanceKind::Local,
            SpanLevel::CharacterStyle | SpanLevel::ParagraphStyle => AtProvenanceKind::Inherited,
            SpanLevel::Document => AtProvenanceKind::Default,
        }
    }

    /// The localized line for this level, given the resolved value and the
    /// style name it came from.
    #[must_use]
    pub fn text(self, value: Option<&str>, source: Option<&str>) -> String {
        let value = value.unwrap_or_default().to_string();
        match self {
            SpanLevel::Direct => fl!("span-dialog-prov-direct"),
            SpanLevel::CharacterStyle => fl!(
                "span-dialog-prov-char-style",
                source = source.unwrap_or_default().to_string(),
                value = value
            ),
            SpanLevel::ParagraphStyle => fl!(
                "span-dialog-prov-para-style",
                source = source.unwrap_or_default().to_string(),
                value = value
            ),
            SpanLevel::Document => fl!("span-dialog-prov-document", value = value),
        }
    }

    /// Whether this level offers a Reset — only direct formatting does.
    #[must_use]
    pub fn is_resettable(self) -> bool {
        self == SpanLevel::Direct
    }
}

/// Which level a property resolves from, given whether the run marks it and
/// which styles are in play.
///
/// The mark wins whenever it is present; below that the character style is
/// consulted before the paragraph style, and the document default is the floor.
#[must_use]
pub(super) fn level_of(
    marked: bool,
    char_style: Option<&str>,
    para_style: Option<&str>,
) -> SpanLevel {
    if marked {
        SpanLevel::Direct
    } else if char_style.is_some() {
        SpanLevel::CharacterStyle
    } else if para_style.is_some() {
        SpanLevel::ParagraphStyle
    } else {
        SpanLevel::Document
    }
}

/// The header's selection summary.
#[must_use]
pub(super) fn selection_summary(
    chars: usize,
    char_style: Option<&str>,
    marks: &SpanMarks,
) -> String {
    match char_style {
        Some(name) => fl!(
            "span-dialog-selection-styled",
            chars = chars as i64,
            style = name.to_string(),
            direct = marks.direct_count() as i64
        ),
        None => fl!(
            "span-dialog-selection",
            chars = chars as i64,
            direct = marks.direct_count() as i64
        ),
    }
}

#[cfg(test)]
#[path = "tabs_tests.rs"]
mod tests;
