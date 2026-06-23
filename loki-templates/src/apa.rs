// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 AppThere Loki contributors

//! APA 7th-edition student paper template.

use loki_doc_model::document::Document;
use loki_doc_model::style::props::para_props::ParagraphAlignment::{Center, Left};

use crate::helpers::{Char, Para, assemble, letter_layout, p, style};

const SERIF: &str = "Times New Roman";

/// Builds the APA 7 paper template (Times New Roman 12 pt, double-spaced,
/// 1-inch margins, title-page block and level-1/2/3 headings).
pub(crate) fn build() -> Document {
    let body_char = || Char {
        font: Some(SERIF),
        size: Some(12.0),
        ..Default::default()
    };
    let dbl = || Para {
        line: Some(2.0),
        ..Default::default()
    };

    let styles = vec![
        style(
            "Normal",
            "Normal",
            None,
            None,
            &body_char(),
            &Para {
                line: Some(2.0),
                indent_first: Some(36.0),
                ..Default::default()
            },
        ),
        // Title page elements are centered, no first-line indent.
        style(
            "APATitle",
            "Title",
            Some("Normal"),
            Some("Normal"),
            &Char {
                font: Some(SERIF),
                size: Some(12.0),
                bold: true,
                ..Default::default()
            },
            &Para {
                align: Some(Center),
                line: Some(2.0),
                space_before: Some(96.0),
                ..Default::default()
            },
        ),
        style(
            "APACenter",
            "Title Page Line",
            Some("Normal"),
            Some("APACenter"),
            &body_char(),
            &Para {
                align: Some(Center),
                line: Some(2.0),
                ..Default::default()
            },
        ),
        // Headings (APA level 1 centered bold, 2 left bold, 3 left bold italic).
        style(
            "Heading1",
            "Heading 1",
            Some("Normal"),
            Some("Normal"),
            &Char {
                font: Some(SERIF),
                size: Some(12.0),
                bold: true,
                ..Default::default()
            },
            &Para {
                align: Some(Center),
                outline: Some(1),
                line: Some(2.0),
                ..Default::default()
            },
        ),
        style(
            "Heading2",
            "Heading 2",
            Some("Normal"),
            Some("Normal"),
            &Char {
                font: Some(SERIF),
                size: Some(12.0),
                bold: true,
                ..Default::default()
            },
            &Para {
                align: Some(Left),
                outline: Some(2),
                line: Some(2.0),
                ..Default::default()
            },
        ),
        style(
            "Heading3",
            "Heading 3",
            Some("Normal"),
            Some("Normal"),
            &Char {
                font: Some(SERIF),
                size: Some(12.0),
                bold: true,
                italic: true,
                ..Default::default()
            },
            &Para {
                align: Some(Left),
                outline: Some(3),
                line: Some(2.0),
                ..dbl()
            },
        ),
    ];

    let body = vec![
        p("APACenter", ""),
        p("APACenter", ""),
        p("APATitle", "The Title of Your Paper"),
        p("APACenter", "Your Name"),
        p("APACenter", "Department of Psychology, University Name"),
        p("APACenter", "PSY 101: Course Name"),
        p("APACenter", "Instructor Name"),
        p("APACenter", "Due Date"),
        p("Heading1", "The Title of Your Paper"),
        p(
            "Normal",
            "Begin the body of your paper here. The first line of each \
           paragraph is indented half an inch and the whole document is double-spaced \
           in 12-point Times New Roman, as APA 7 requires.",
        ),
        p("Heading2", "A Level 2 Heading"),
        p(
            "Normal",
            "Use the heading styles to structure your sections; they carry \
           the correct APA alignment and emphasis.",
        ),
    ];

    assemble("APA Paper", letter_layout(1.0), styles, body)
}
