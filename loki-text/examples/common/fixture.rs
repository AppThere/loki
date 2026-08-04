// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 AppThere Loki contributors

//! The documents ADR-0017's line-break comparison runs on — **one** definition,
//! shared by the layout half (`styled_linebreak_sweep`) and the DOM half
//! (`styled_linebreak_probe`) through `#[path]`.
//!
//! Shared rather than duplicated because the comparison is only meaningful if
//! both halves lay out the *same* document; two copies of a fixture agree until
//! one is edited, and that edit would read as a rendering difference.
//!
//! # `LB_FIXTURE`
//!
//! * `screenplay` (default) — `loki-templates`' screenplay: named styles,
//!   indents, alignment, one substituted family, and **no** mixed runs within a
//!   paragraph. This is what §5.3 swept.
//! * `mixed` — [`mixed_runs`] below: §5.4's open case, one paragraph per way a
//!   run can differ from the one beside it.
//! * `mixed:<case>` — a single case from that set, for attributing a
//!   disagreement the whole-document count can only detect. The cases are
//!   `plain`, `weight`, `italic`, `size`, `family`, `charstyle` and `spacing`.
//! * `mixed:onerun` — `plain`'s characters with **no run boundary** at all.
//! * `mixed:solo-<case>` — that case's properties over the *whole* paragraph,
//!   with no run beside them.
//!
//! The last two are the controls, and they are what made §5.5 attributable:
//! `plain` isolates "there is a boundary" from "the property changed", `onerun`
//! removes the boundary entirely, and `solo-<case>` separates "these two runs
//! differ" from "this run's face and size differ wherever they appear".

use loki_doc_model::content::attr::NodeAttr;
use loki_doc_model::content::block::{Block, StyledParagraph};
use loki_doc_model::content::inline::{Inline, StyledRun};
use loki_doc_model::document::Document;
use loki_doc_model::layout::page::{PageLayout, PageMargins, PageSize};
use loki_doc_model::layout::section::Section;
use loki_doc_model::loki_primitives::units::Points;
use loki_doc_model::meta::DocumentMeta;
use loki_doc_model::style::catalog::{StyleCatalog, StyleId};
use loki_doc_model::style::char_style::CharacterStyle;
use loki_doc_model::style::para_style::ParagraphStyle;
use loki_doc_model::style::props::char_props::CharProps;
use loki_doc_model::style::props::para_props::{ParaProps, Spacing};

/// The base family. **Deliberately one the host will not have**: it resolves
/// through `resolve_font_name` to the bundled Arimo, so every case exercises
/// substitution rather than only the case that names a second family.
const BASE_FAMILY: &str = "Arial";
/// A second family, mixed *into* a paragraph set in the first.
const MONO_FAMILY: &str = "Courier New";
/// A third, reached only through a named **character** style.
const SERIF_FAMILY: &str = "Times New Roman";

/// The document named by `LB_FIXTURE`, and the name to print for the record.
pub fn from_env() -> (String, Document) {
    let spec = std::env::var("LB_FIXTURE").unwrap_or_else(|_| "screenplay".to_string());
    match spec.split_once(':') {
        Some(("mixed", case)) => (spec.clone(), mixed_runs(Some(case))),
        _ if spec == "mixed" => (spec.clone(), mixed_runs(None)),
        _ => (
            "screenplay".to_string(),
            loki_templates::document("screenplay").unwrap_or_else(|| mixed_runs(None)),
        ),
    }
}

/// Enough text on each side of the styled run that the paragraph wraps several
/// times and the run sits mid-line rather than at a break.
const LEAD: &str = "The column narrows and the type has to decide where to break, so \
this sentence runs on until it is long enough that the ";
const TAIL: &str = " sits somewhere in the middle of a line rather than politely at \
the end of one, and then it keeps running for a while longer so that the tail wraps too.";

/// One paragraph per way a run can differ from the run beside it.
///
/// Each is the *same* text with the *same* base style; only the middle run's
/// property changes. A count that agrees across all of them agrees about mixed
/// metrics; a count that disagrees is attributable by re-running one case.
pub fn mixed_runs(only: Option<&str>) -> Document {
    let cases: Vec<(&str, &str, CharProps, Option<&str>)> = vec![
        // **The control.** Same text, same base style, no property change at
        // all — so the middle run is not a different run. If this case
        // disagrees, nothing below it is evidence about *mixed* runs: the
        // disagreement is in the base, and the mixed cases inherit it.
        ("plain", "ordinary words here", CharProps::default(), None),
        (
            "weight",
            "bold words here",
            CharProps {
                bold: Some(true),
                font_weight: Some(700),
                ..Default::default()
            },
            None,
        ),
        (
            "italic",
            "italic words here",
            CharProps {
                italic: Some(true),
                ..Default::default()
            },
            None,
        ),
        (
            "size",
            "eighteen point words",
            CharProps {
                font_size: Some(Points::new(18.0)),
                ..Default::default()
            },
            None,
        ),
        (
            "family",
            "monospaced words here",
            CharProps {
                font_name: Some(MONO_FAMILY.to_string()),
                ..Default::default()
            },
            None,
        ),
        // Formatting reached through the catalog rather than stated on the run:
        // the DOM path has to resolve character styles too, not only paragraph
        // ones, and a run that carried its properties directly would not say so.
        (
            "charstyle",
            "styled words here",
            CharProps::default(),
            Some("Emphasis"),
        ),
        (
            "spacing",
            "tracked out words",
            CharProps {
                letter_spacing: Some(Points::new(1.5)),
                ..Default::default()
            },
            None,
        ),
    ];

    // A second control, below the first: the *same characters* as `plain`, but
    // as one inline rather than three. `plain` still crosses two run boundaries
    // — the resolver emits a span per run even when the runs resolve alike — so
    // it cannot separate "the property changed" from "there is a boundary here
    // at all". This one has no boundary.
    if only == Some("onerun") {
        return assemble(vec![Block::StyledPara(StyledParagraph {
            style_id: Some(StyleId::new("Normal")),
            direct_para_props: None,
            direct_char_props: None,
            inlines: vec![Inline::Str(format!("{LEAD}ordinary words here{TAIL}"))],
            attr: NodeAttr::default(),
        })]);
    }

    // `solo-<case>`: the whole paragraph in that case's properties, with no run
    // beside it. The third control, and the one that separates "these two runs
    // disagree because they are *different*" from "this run's face and size
    // disagree wherever it appears". A case that disagrees mixed *and* solo is
    // not evidence about mixing at all.
    if let Some(solo) = only.and_then(|c| c.strip_prefix("solo-"))
        && let Some((_, text, props, style)) = cases.iter().find(|(n, ..)| *n == solo)
    {
        return assemble(vec![Block::StyledPara(StyledParagraph {
            style_id: Some(StyleId::new("Normal")),
            direct_para_props: None,
            direct_char_props: None,
            inlines: vec![Inline::StyledRun(StyledRun {
                style_id: style.map(StyleId::new),
                direct_props: Some(Box::new(props.clone())),
                content: vec![Inline::Str(format!("{LEAD}{text}{TAIL}"))],
                attr: NodeAttr::default(),
            })],
            attr: NodeAttr::default(),
        })]);
    }

    let blocks: Vec<Block> = cases
        .into_iter()
        .filter(|(name, ..)| only.is_none_or(|c| c == *name))
        .map(|(_, text, props, style)| {
            Block::StyledPara(StyledParagraph {
                style_id: Some(StyleId::new("Normal")),
                direct_para_props: None,
                direct_char_props: None,
                inlines: vec![
                    Inline::Str(LEAD.to_string()),
                    Inline::StyledRun(StyledRun {
                        style_id: style.map(StyleId::new),
                        direct_props: Some(Box::new(props)),
                        content: vec![Inline::Str(text.to_string())],
                        attr: NodeAttr::default(),
                    }),
                    Inline::Str(TAIL.to_string()),
                ],
                attr: NodeAttr::default(),
            })
        })
        .collect();

    assemble(blocks)
}

/// A single-section document carrying `blocks`, a `Normal` paragraph style and
/// the `Emphasis` character style the `charstyle` case names.
fn assemble(blocks: Vec<Block>) -> Document {
    let mut catalog = StyleCatalog::new();
    catalog.paragraph_styles.insert(
        StyleId::new("Normal"),
        ParagraphStyle {
            id: StyleId::new("Normal"),
            display_name: Some("Normal".to_string()),
            parent: None,
            linked_char_style: None,
            next_style_id: None,
            para_props: ParaProps {
                space_after: Some(Spacing::Exact(Points::new(12.0))),
                ..Default::default()
            },
            char_props: CharProps {
                font_name: Some(BASE_FAMILY.to_string()),
                font_size: Some(Points::new(12.0)),
                ..Default::default()
            },
            is_default: true,
            is_custom: false,
            extensions: Default::default(),
        },
    );
    catalog.default_paragraph_style = Some(StyleId::new("Normal"));
    catalog.character_styles.insert(
        StyleId::new("Emphasis"),
        CharacterStyle {
            id: StyleId::new("Emphasis"),
            display_name: Some("Emphasis".to_string()),
            parent: None,
            char_props: CharProps {
                font_name: Some(SERIF_FAMILY.to_string()),
                font_size: Some(Points::new(14.0)),
                bold: Some(true),
                font_weight: Some(700),
                ..Default::default()
            },
            extensions: Default::default(),
        },
    );

    let margin = Points::new(72.0);
    let layout = PageLayout {
        page_size: PageSize::letter(),
        margins: PageMargins {
            top: margin,
            bottom: margin,
            left: margin,
            right: margin,
            header: Points::new(36.0),
            footer: Points::new(36.0),
            gutter: Points::new(0.0),
        },
        ..PageLayout::default()
    };

    Document {
        meta: DocumentMeta {
            title: Some("mixed runs".to_string()),
            ..Default::default()
        },
        styles: catalog,
        sections: vec![Section::with_layout_and_blocks(layout, blocks)],
        settings: None,
        comments: Vec::new(),
        source: None,
    }
}
