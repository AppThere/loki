// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 AppThere Loki contributors

//! Text content carried inside a shape (a text box, a presentation
//! placeholder, or a drawing label).
//!
//! This is a deliberately small rich-text model — enough for shape text in
//! presentations and drawings — independent of `loki-doc-model`'s
//! word-processing model. It can be enriched (tabs, fields, inline images)
//! without affecting that model.

use loki_primitives::color::DocumentColor;

#[cfg(feature = "serde")]
use serde::{Deserialize, Serialize};

/// The text body of a shape: a vertical stack of paragraphs plus an anchor.
#[derive(Debug, Clone, PartialEq, Default)]
#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
pub struct TextBody {
    /// Paragraphs in top-to-bottom order.
    pub paragraphs: Vec<TextParagraph>,
    /// Vertical alignment of the block within the shape frame.
    pub anchor: VerticalAnchor,
}

impl TextBody {
    /// Builds a single-paragraph, single-run body from plain text.
    pub fn plain(text: impl Into<String>) -> Self {
        Self {
            paragraphs: vec![TextParagraph::plain(text)],
            anchor: VerticalAnchor::default(),
        }
    }

    /// Whether there is no text at all.
    pub fn is_empty(&self) -> bool {
        self.paragraphs.iter().all(|p| p.runs.is_empty())
    }
}

/// One paragraph of shape text.
#[derive(Debug, Clone, PartialEq, Default)]
#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
pub struct TextParagraph {
    /// The styled text runs making up the paragraph.
    pub runs: Vec<TextRun>,
    /// Horizontal alignment.
    pub align: TextAlign,
    /// Outline / bullet indent level (0 = top level).
    pub level: u8,
}

impl TextParagraph {
    /// Builds a paragraph with a single unstyled run.
    pub fn plain(text: impl Into<String>) -> Self {
        Self {
            runs: vec![TextRun {
                text: text.into(),
                props: TextRunProps::default(),
            }],
            align: TextAlign::default(),
            level: 0,
        }
    }

    /// The concatenated plain text of all runs.
    pub fn text(&self) -> String {
        self.runs.iter().map(|r| r.text.as_str()).collect()
    }
}

/// A run of text sharing one set of character properties.
#[derive(Debug, Clone, PartialEq, Default)]
#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
pub struct TextRun {
    /// The run's text.
    pub text: String,
    /// Character formatting.
    pub props: TextRunProps,
}

/// Character-level formatting for a [`TextRun`].
#[derive(Debug, Clone, PartialEq, Default)]
#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
pub struct TextRunProps {
    /// Bold weight.
    pub bold: bool,
    /// Italic style.
    pub italic: bool,
    /// Underline.
    pub underline: bool,
    /// Font size in points; `None` inherits from the placeholder/theme default.
    pub font_size_pt: Option<f64>,
    /// Text color; `None` inherits.
    pub color: Option<DocumentColor>,
    /// Font family name; `None` inherits.
    pub font_family: Option<String>,
}

/// Horizontal text alignment.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
pub enum TextAlign {
    /// Left-aligned.
    #[default]
    Left,
    /// Centered.
    Center,
    /// Right-aligned.
    Right,
    /// Justified.
    Justify,
}

/// Vertical anchoring of a [`TextBody`] within its shape.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
pub enum VerticalAnchor {
    /// Anchored to the top of the frame.
    #[default]
    Top,
    /// Centered vertically.
    Middle,
    /// Anchored to the bottom.
    Bottom,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn plain_body_round_trips_text() {
        let body = TextBody::plain("Hello");
        assert_eq!(body.paragraphs.len(), 1);
        assert_eq!(body.paragraphs[0].text(), "Hello");
        assert!(!body.is_empty());
    }

    #[test]
    fn empty_body_is_empty() {
        assert!(TextBody::default().is_empty());
    }

    #[test]
    fn paragraph_concatenates_runs() {
        let para = TextParagraph {
            runs: vec![
                TextRun {
                    text: "foo ".into(),
                    props: TextRunProps::default(),
                },
                TextRun {
                    text: "bar".into(),
                    props: TextRunProps {
                        bold: true,
                        ..Default::default()
                    },
                },
            ],
            align: TextAlign::Center,
            level: 1,
        };
        assert_eq!(para.text(), "foo bar");
    }
}
