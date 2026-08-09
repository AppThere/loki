// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 AppThere Loki contributors

//! `style:page-usage` — which pages of a spread a layout applies to, and the
//! ODF codec for it. Split from `page.rs` at the 300-line ceiling, on a real
//! seam: this is one self-contained type plus its own string mapping, where the
//! rest of that file is geometry.

/// Which pages of a spread a page layout applies to (ODF `style:page-usage`,
/// ODF 1.3 §19.469).
///
/// # The two formats disagree about *where* this lives, not about what it means
///
/// ODF puts it on the page layout, so a document can mirror one page style and
/// not another. OOXML has no per-section equivalent: `w:mirrorMargins` is a
/// single flag in `settings.xml` for the whole document. The model takes the
/// richer of the two — a per-layout property — because a document-wide flag can
/// always be expressed as "every layout mirrors", while the reverse loses
/// information that ODF round-trips.
///
/// Collapsing happens at the DOCX boundary, which is the only place it is
/// lossy, and the loss is the format's rather than the model's.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub enum PageUsage {
    /// Applies to every page; margins are not mirrored. The default.
    #[default]
    All,
    /// Mirrored — even (verso) pages swap left and right margins so the inside
    /// margin faces the binding on both pages of a spread. The ODF spelling of
    /// OOXML's `w:mirrorMargins`.
    Mirrored,
    /// Applies to left (verso) pages only.
    Left,
    /// Applies to right (recto) pages only.
    Right,
}

impl PageUsage {
    /// Whether margins alternate by page parity under this usage.
    ///
    /// **The single derivation of the question the paginator asks.** `Left` and
    /// `Right` restrict *which* pages a layout is used for rather than swapping
    /// margins within it, so only `Mirrored` answers yes — a distinction that is
    /// easy to get wrong at a call site and is therefore not made at one.
    #[must_use]
    pub fn mirrors_margins(self) -> bool {
        matches!(self, Self::Mirrored)
    }

    /// Parses an ODF `style:page-usage` value; unknown values are [`Self::All`].
    ///
    /// Unknown-is-default rather than an error: an unreadable page usage is a
    /// document that should still open, and ODF's own default for the attribute
    /// is `all`.
    #[must_use]
    pub fn from_odf(value: &str) -> Self {
        match value {
            "mirrored" => Self::Mirrored,
            "left" => Self::Left,
            "right" => Self::Right,
            _ => Self::All,
        }
    }

    /// The ODF `style:page-usage` value for this usage.
    #[must_use]
    pub fn as_odf(self) -> &'static str {
        match self {
            Self::All => "all",
            Self::Mirrored => "mirrored",
            Self::Left => "left",
            Self::Right => "right",
        }
    }
}

#[cfg(test)]
#[path = "page_usage_tests.rs"]
mod tests;
