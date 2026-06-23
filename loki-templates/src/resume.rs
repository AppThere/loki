// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 AppThere Loki contributors

//! Basic single-column resume template.

use loki_doc_model::document::Document;
use loki_doc_model::style::props::para_props::ParagraphAlignment::Center;

use crate::helpers::{Char, Para, assemble, letter_layout, p, style};

const SANS: &str = "Arial";

/// Builds the basic resume template (clean sans-serif, name banner, section
/// headings, and entry styles) on US Letter with 0.8-inch margins.
pub(crate) fn build() -> Document {
    let body = || Char {
        font: Some(SANS),
        size: Some(10.5),
        ..Default::default()
    };

    let styles = vec![
        style(
            "Normal",
            "Normal",
            None,
            None,
            &body(),
            &Para {
                space_after: Some(4.0),
                line: Some(1.15),
                ..Default::default()
            },
        ),
        // Name banner.
        style(
            "ResumeName",
            "Name",
            Some("Normal"),
            Some("ResumeContact"),
            &Char {
                font: Some(SANS),
                size: Some(24.0),
                bold: true,
                ..Default::default()
            },
            &Para {
                align: Some(Center),
                space_after: Some(2.0),
                ..Default::default()
            },
        ),
        // Contact line under the name.
        style(
            "ResumeContact",
            "Contact",
            Some("Normal"),
            Some("Normal"),
            &Char {
                font: Some(SANS),
                size: Some(9.5),
                ..Default::default()
            },
            &Para {
                align: Some(Center),
                space_after: Some(10.0),
                ..Default::default()
            },
        ),
        // Section heading (Experience, Education, …).
        style(
            "ResumeSection",
            "Section Heading",
            Some("Normal"),
            Some("Normal"),
            &Char {
                font: Some(SANS),
                size: Some(12.0),
                bold: true,
                ..Default::default()
            },
            &Para {
                outline: Some(1),
                space_before: Some(10.0),
                space_after: Some(4.0),
                ..Default::default()
            },
        ),
        // Entry title (job / degree) and its detail line.
        style(
            "ResumeEntry",
            "Entry Title",
            Some("Normal"),
            Some("Normal"),
            &Char {
                font: Some(SANS),
                size: Some(11.0),
                bold: true,
                ..Default::default()
            },
            &Para {
                space_after: Some(1.0),
                ..Default::default()
            },
        ),
        style(
            "ResumeDetail",
            "Entry Detail",
            Some("Normal"),
            Some("ResumeDetail"),
            &Char {
                font: Some(SANS),
                size: Some(10.0),
                italic: true,
                ..Default::default()
            },
            &Para {
                space_after: Some(4.0),
                ..Default::default()
            },
        ),
    ];

    let body_blocks = vec![
        p("ResumeName", "Your Name"),
        p(
            "ResumeContact",
            "City, State \u{2022} email@example.com \u{2022} (555) 123-4567 \u{2022} linkedin.com/in/you",
        ),
        p("ResumeSection", "Experience"),
        p("ResumeEntry", "Job Title \u{2014} Company"),
        p("ResumeDetail", "Location \u{2022} Start Date – End Date"),
        p(
            "Normal",
            "Describe an accomplishment with a measurable result. Lead with a \
           strong verb and quantify the impact where you can.",
        ),
        p("ResumeSection", "Education"),
        p("ResumeEntry", "Degree, Field of Study"),
        p("ResumeDetail", "University Name \u{2022} Graduation Year"),
        p("ResumeSection", "Skills"),
        p(
            "Normal",
            "List relevant tools, languages, and competencies, separated by commas.",
        ),
    ];

    assemble("Resume", letter_layout(0.8), styles, body_blocks)
}
