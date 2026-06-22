// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 AppThere Loki contributors

//! Blank document with Markdown-inspired styles.

use loki_doc_model::document::Document;
use loki_doc_model::style::para_style::ParagraphStyle;
use loki_doc_model::style::props::para_props::ParagraphAlignment;

use crate::helpers::{Char, Para, assemble, heading_block, letter_layout, p, style};

const SANS: &str = "Arial";
const MONO: &str = "Courier New";

/// Builds the Markdown-styled blank template.
pub(crate) fn build() -> Document {
    let styles = vec![
        style(
            "Normal",
            "Normal",
            None,
            None,
            &Char {
                font: Some(SANS),
                size: Some(11.0),
                ..Default::default()
            },
            &Para {
                line: Some(1.45),
                space_after: Some(8.0),
                ..Default::default()
            },
        ),
        heading("Heading1", "Heading 1", 26.0, 1, 18.0, 6.0),
        heading("Heading2", "Heading 2", 20.0, 2, 16.0, 5.0),
        heading("Heading3", "Heading 3", 16.0, 3, 14.0, 4.0),
        heading("Heading4", "Heading 4", 13.0, 4, 12.0, 4.0),
        style(
            "Blockquote",
            "Block Quote",
            Some("Normal"),
            Some("Blockquote"),
            &Char {
                font: Some(SANS),
                size: Some(11.0),
                italic: true,
                ..Default::default()
            },
            &Para {
                indent_left: Some(24.0),
                space_after: Some(8.0),
                line: Some(1.45),
                ..Default::default()
            },
        ),
        style(
            "CodeBlock",
            "Code Block",
            Some("Normal"),
            Some("CodeBlock"),
            &Char {
                font: Some(MONO),
                size: Some(10.0),
                ..Default::default()
            },
            &Para {
                indent_left: Some(12.0),
                space_after: Some(8.0),
                ..Default::default()
            },
        ),
    ];

    let body = vec![
        heading_block(1, "Document Title"),
        p(
            "Normal",
            "Start writing here. This template ships paragraph and heading \
           styles inspired by rendered Markdown, so structure stays consistent.",
        ),
        heading_block(2, "A Section"),
        p(
            "Normal",
            "Body text uses a clean sans-serif face with comfortable line \
           spacing. Apply the heading styles from the ribbon to build an outline.",
        ),
        p("Blockquote", "Block quotes are indented and italicised."),
        p("CodeBlock", "code blocks use a monospace face"),
    ];

    assemble("Markdown Document", letter_layout(1.0), styles, body)
}

/// A heading style with the given size, outline level, before/after spacing.
fn heading(
    id: &str,
    name: &str,
    size: f64,
    outline: u8,
    before: f64,
    after: f64,
) -> ParagraphStyle {
    style(
        id,
        name,
        Some("Normal"),
        Some("Normal"),
        &Char {
            font: Some(SANS),
            size: Some(size),
            bold: true,
            ..Default::default()
        },
        &Para {
            align: Some(ParagraphAlignment::Left),
            outline: Some(outline),
            space_before: Some(before),
            space_after: Some(after),
            ..Default::default()
        },
    )
}
