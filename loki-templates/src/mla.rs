// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 AppThere Loki contributors

//! MLA 9th-edition paper template.

use loki_doc_model::document::Document;
use loki_doc_model::style::props::para_props::ParagraphAlignment::Center;

use crate::helpers::{Char, Para, assemble, letter_layout, p, style};

const SERIF: &str = "Times New Roman";

/// Builds the MLA 9 paper template (Times New Roman 12 pt, double-spaced,
/// 1-inch margins, four-line heading block, centered title, half-inch
/// first-line indent, and a hanging-indent Works Cited style).
pub(crate) fn build() -> Document {
    let body = || Char {
        font: Some(SERIF),
        size: Some(12.0),
        ..Default::default()
    };

    let styles = vec![
        // Body: double-spaced, half-inch first-line indent.
        style(
            "Normal",
            "Normal",
            None,
            None,
            &body(),
            &Para {
                line: Some(2.0),
                indent_first: Some(36.0),
                ..Default::default()
            },
        ),
        // The name/instructor/course/date block: double-spaced, no indent.
        style(
            "MLAHeading",
            "Heading Block",
            Some("Normal"),
            Some("MLAHeading"),
            &body(),
            &Para {
                line: Some(2.0),
                ..Default::default()
            },
        ),
        // Centered title, no indent.
        style(
            "MLATitle",
            "Title",
            Some("Normal"),
            Some("Normal"),
            &body(),
            &Para {
                align: Some(Center),
                line: Some(2.0),
                ..Default::default()
            },
        ),
        // Works Cited entries: hanging indent of half an inch.
        style(
            "WorksCited",
            "Works Cited Entry",
            Some("Normal"),
            Some("WorksCited"),
            &body(),
            &Para {
                line: Some(2.0),
                indent_left: Some(36.0),
                hanging: Some(36.0),
                ..Default::default()
            },
        ),
    ];

    let body_blocks = vec![
        p("MLAHeading", "Your Name"),
        p("MLAHeading", "Instructor Name"),
        p("MLAHeading", "Course Number"),
        p("MLAHeading", "Day Month Year"),
        p("MLATitle", "The Title of Your Paper"),
        p(
            "Normal",
            "Begin your essay here. MLA style uses 12-point Times New Roman, \
           double spacing throughout, and a half-inch first-line indent on each \
           paragraph. The four-line heading block above is flush left.",
        ),
        p("MLATitle", "Works Cited"),
        p(
            "WorksCited",
            "Author Last, First. Title of Source. Publisher, Year.",
        ),
    ];

    assemble("MLA Paper", letter_layout(1.0), styles, body_blocks)
}
