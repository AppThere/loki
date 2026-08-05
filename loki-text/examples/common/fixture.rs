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
        // `advances[:<scale>]` — the §5.6 measuring document, at an optional
        // font-size multiplier.
        Some(("advances", scale)) => (spec.clone(), advance_runs(scale.parse().unwrap_or(1.0))),
        _ if spec == "advances" => (spec.clone(), advance_runs(1.0)),
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

/// The **advance** fixture (ADR-0017 §5.6): one paragraph per resolved style,
/// all carrying the *same* string, at `scale` × their natural size.
///
/// Not a line-breaking document — a measuring one. Each paragraph is a single
/// run, so each has exactly one advance, and both paths report that advance for
/// the same characters in the same resolved style. The cases are in the order
/// [`ADVANCE_CASES`] names them.
pub fn advance_runs(scale: f64) -> Document {
    let blocks = ADVANCE_CASES
        .iter()
        .map(|(name, props, style)| {
            let mut props = props();
            // Scaled here rather than in `assemble_scaled`, which only knows the
            // catalog: a directly-stated size or tracking is the run's own.
            props.font_size = props.font_size.map(|s| Points::new(s.value() * scale));
            props.letter_spacing = props.letter_spacing.map(|s| Points::new(s.value() * scale));
            Block::StyledPara(StyledParagraph {
                style_id: Some(StyleId::new("Normal")),
                direct_para_props: None,
                direct_char_props: None,
                inlines: vec![Inline::StyledRun(StyledRun {
                    style_id: style.map(StyleId::new),
                    direct_props: Some(Box::new(props)),
                    content: vec![Inline::Str(advance_text(name).to_string())],
                    attr: NodeAttr::default(),
                })],
                attr: NodeAttr::default(),
            })
        })
        .collect();
    assemble_scaled(blocks, scale)
}

/// The one string every advance case sets, chosen for a spread of glyph widths
/// and no leading or trailing space (which would hang, and hang differently).
pub const ADVANCE_TEXT: &str = "Handgloves quick brown fox jumps over lazy dogs";

/// `(name, direct properties, character style)` for each advance case, in
/// document order — the caller needs the names to label what it measures.
#[expect(
    clippy::type_complexity,
    reason = "a table read once by two probes; a struct would be more names than facts"
)]
pub fn advance_cases() -> &'static [(&'static str, fn() -> CharProps, Option<&'static str>)] {
    ADVANCE_CASES
}

#[expect(clippy::type_complexity, reason = "see `advance_cases`")]
static ADVANCE_CASES: &[(&str, fn() -> CharProps, Option<&str>)] = &[
    ("base", CharProps::default, None),
    ("bold", bold_props, None),
    ("italic", italic_props, None),
    ("mono", mono_props, None),
    ("tracked", tracked_props, None),
    ("emph", CharProps::default, Some("Emphasis")),
    // `emph` is Tinos **bold** reached through a character style, and it is the
    // one case that disagreed. These two split what that confounds: `serif` is
    // the same face unbolded, `serifbold` is the same face bolded but stated
    // directly on the run. Between them they say whether the difference is the
    // weight, the face, or the catalog.
    ("serif", serif_props, None),
    ("serifbold", serif_bold_props, None),
    // The **installed** metric-compatible serif, named directly so no
    // substitution happens. Tinos and Liberation Serif are both metric-clones of
    // Times New Roman and are near-identical; if the DOM's Tinos Bold matches
    // *this* rather than the bundled Tinos Bold, the DOM is resolving the family
    // to the system face.
    ("libserif", lib_serif_props, None),
    ("libserifbold", lib_serif_bold_props, None),
    ("kernfree", lib_serif_bold_props, None),
    ("kernheavy", lib_serif_bold_props, None),
];

/// Cases that need their own string rather than [`ADVANCE_TEXT`], for asking
/// whether the bold-serif difference is **kerning**: a run of one repeated
/// letter has no kern pairs at all, a run of `AV`/`Ta`/`Wo` is almost nothing
/// but kern pairs. If the first agrees and the second does not, the two paths
/// differ in what they apply, not in what face they picked.
static ADVANCE_TEXT_OVERRIDES: &[(&str, &str)] = &[
    ("kernfree", "nnnnnnnnnnnnnnnnnnnnnnnnnnnnnnnnnnnnnnnnnnnnnn"),
    (
        "kernheavy",
        "AV Ta Wo AV Ta Wo AV Ta Wo AV Ta Wo AV Ta Wo A",
    ),
];

/// The string case `name` sets.
fn advance_text(name: &str) -> &'static str {
    ADVANCE_TEXT_OVERRIDES
        .iter()
        .find(|(n, _)| *n == name)
        .map_or(ADVANCE_TEXT, |(_, t)| t)
}

/// A face the host has installed, so nothing substitutes it.
const INSTALLED_SERIF: &str = "Liberation Serif";

fn lib_serif_props() -> CharProps {
    CharProps {
        font_name: Some(INSTALLED_SERIF.to_string()),
        font_size: Some(Points::new(14.0)),
        ..Default::default()
    }
}

fn lib_serif_bold_props() -> CharProps {
    CharProps {
        bold: Some(true),
        font_weight: Some(700),
        ..lib_serif_props()
    }
}

fn bold_props() -> CharProps {
    CharProps {
        bold: Some(true),
        font_weight: Some(700),
        ..Default::default()
    }
}

fn italic_props() -> CharProps {
    CharProps {
        italic: Some(true),
        ..Default::default()
    }
}

fn mono_props() -> CharProps {
    CharProps {
        font_name: Some(MONO_FAMILY.to_string()),
        ..Default::default()
    }
}

fn serif_props() -> CharProps {
    CharProps {
        font_name: Some(SERIF_FAMILY.to_string()),
        font_size: Some(Points::new(14.0)),
        ..Default::default()
    }
}

fn serif_bold_props() -> CharProps {
    CharProps {
        bold: Some(true),
        font_weight: Some(700),
        ..serif_props()
    }
}

fn tracked_props() -> CharProps {
    CharProps {
        letter_spacing: Some(Points::new(1.5)),
        ..Default::default()
    }
}

/// A single-section document carrying `blocks`, a `Normal` paragraph style and
/// the `Emphasis` character style the `charstyle` case names.
fn assemble(blocks: Vec<Block>) -> Document {
    assemble_scaled(blocks, 1.0)
}

/// [`assemble`] with every font size (and the tracked case's letter-spacing,
/// which its caller scales) multiplied by `scale`.
///
/// The scale exists for §5.6's advance measurement: a difference of order 0.1 %
/// is a fraction of a pixel at 12 pt and several pixels at 96 pt, and whether it
/// *grows with the size* is what separates a rounding from a metrics difference.
fn assemble_scaled(blocks: Vec<Block>, scale: f64) -> Document {
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
                font_size: Some(Points::new(12.0 * scale)),
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
                font_size: Some(Points::new(14.0 * scale)),
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
