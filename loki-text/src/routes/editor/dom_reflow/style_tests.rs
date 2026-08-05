// SPDX-License-Identifier: Apache-2.0

//! Tests for the DOM reflow view's **resolved** property → CSS mapping
//! (ADR-0017 §5.1).
//!
//! These take `loki_layout`'s resolved types, which is the point: the values
//! under test are the same ones the canvas path shapes with, so a mapping error
//! here is a difference between the two paths and nothing else.

use super::super::content::FamilyMap;
use super::{css_layout_color, resolved_para_css, span_css};
use loki_layout::color::LayoutColor;
use loki_layout::para::{ResolvedParaProps, StyleSpan};

/// A resolved span, obtained the way the view obtains one: by running a real
/// paragraph through `loki_layout`'s own flattener.
///
/// Constructing a `StyleSpan` field-by-field would be a second statement of what
/// "resolved" means, and it would keep compiling after the real resolver started
/// producing something different — which is precisely the drift these tests
/// exist to catch.
fn resolved_span() -> StyleSpan {
    use loki_doc_model::content::block::StyledParagraph;
    use loki_doc_model::content::inline::Inline;
    use loki_doc_model::style::catalog::StyleCatalog;

    let para = StyledParagraph {
        style_id: None,
        direct_para_props: None,
        direct_char_props: None,
        inlines: vec![Inline::Str("resolved".into())],
        attr: Default::default(),
    };
    let mut notes = 0u32;
    let (_, spans, _, _) = loki_layout::resolve::flatten_paragraph_with_base(
        &para,
        &StyleCatalog::new(),
        &mut notes,
        None,
        loki_layout::RevisionDisplay::default(),
    );
    spans.into_iter().next().expect("one run")
}

/// **Every resolved property is written out.** A resolved value is definite, so
/// nothing is left for CSS inheritance to supply — which is what stops Stylo's
/// cascade and ours having to agree.
#[test]
fn a_resolved_run_emits_every_property_it_carries() {
    let css = span_css(&resolved_span(), &FamilyMap::new());
    for decl in [
        "font-size:",
        "font-weight:",
        "font-style:",
        "color:",
        "text-decoration:",
    ] {
        assert!(css.contains(decl), "a default run omitted {decl}: {css}");
    }
}

/// **The numeric weight, not the boolean.** `StyleSpan` carries both, and its
/// own docs say the numeric one supersedes `bold` when a `font_weight` style set
/// it — a semibold run written as `bold` would render at 700.
#[test]
fn the_numeric_weight_is_used_not_the_bold_flag() {
    let semibold = StyleSpan {
        weight: 600,
        bold: false,
        ..resolved_span()
    };
    assert!(span_css(&semibold, &FamilyMap::new()).contains("font-weight: 600"));

    // And the boolean does not override it: a span flagged bold but resolved to
    // 600 is still 600.
    let both = StyleSpan {
        weight: 600,
        bold: true,
        ..resolved_span()
    };
    assert!(
        span_css(&both, &FamilyMap::new()).contains("font-weight: 600"),
        "the boolean overrode the resolved numeric weight"
    );
}

/// Underline and strikethrough share one CSS property; two declarations would
/// make the second win and the first vanish.
#[test]
fn underline_and_strikethrough_combine_into_one_declaration() {
    let s = StyleSpan {
        underline: Some(loki_layout::para::UnderlineStyle::Single),
        strikethrough: Some(loki_layout::para::StrikethroughStyle::Single),
        ..resolved_span()
    };
    let css = span_css(&s, &FamilyMap::new());
    assert_eq!(
        css.matches("text-decoration").count(),
        1,
        "two text-decoration declarations — the first is dead: {css}"
    );
    assert!(
        css.contains("underline") && css.contains("line-through"),
        "{css}"
    );

    // The inverse: a run with neither says `none` explicitly rather than
    // omitting it, so an ancestor's decoration cannot leak in.
    assert!(span_css(&resolved_span(), &FamilyMap::new()).contains("text-decoration: none"));
}

/// **Alpha survives.** A run at less than full opacity is a real thing in the
/// model, and flattening it to `rgb()` would paint a watermark as body text.
#[test]
fn colour_alpha_is_carried_through() {
    let faint = css_layout_color(LayoutColor {
        r: 1.0,
        g: 0.0,
        b: 0.0,
        a: 0.25,
    });
    assert!(faint.starts_with("rgba("), "{faint}");
    assert!(faint.contains("0.25"), "alpha was dropped: {faint}");
    assert!(faint.contains("255, 0, 0"), "{faint}");
}

/// A family name is quoted, because names contain spaces and an unquoted
/// `Liberation Sans` is two keywords rather than one family.
#[test]
fn a_family_name_is_quoted() {
    let s = StyleSpan {
        font_name: Some("Liberation Sans".into()),
        ..resolved_span()
    };
    assert!(span_css(&s, &FamilyMap::new()).contains("font-family: 'Liberation Sans'"));
}

/// **Sizes stay in points.** Converting to px would restate the 96/72 ratio
/// Blitz already applies, and the two copies would drift.
#[test]
fn sizes_are_emitted_in_points() {
    let css = span_css(&resolved_span(), &FamilyMap::new());
    assert!(css.contains("12pt"), "{css}");
    assert!(!css.contains("px"), "a size was converted to px: {css}");
}

/// **Kerning follows the document, and the document's default is off.**
///
/// `para_build` disables the `kern` feature for anything but `Some(true)`,
/// because Word and LibreOffice default pair kerning off. CSS defaults it *on*,
/// so a run that says nothing must say `none` here — measured (ADR-0017 §5.6):
/// left unstated, the DOM path set the same string 0.13 % narrower in serif
/// prose and 7.7 % narrower on a kern-heavy one.
#[test]
fn kerning_is_off_unless_the_run_asks_for_it() {
    assert_eq!(super::span_font_features(&resolved_span()), "\"kern\" 0");

    // The inversion: a run that *does* ask for kerning gets it, or the rule is
    // "never kern" rather than "follow the document".
    let kerned = StyleSpan {
        kerning: Some(true),
        ..resolved_span()
    };
    assert_eq!(super::span_font_features(&kerned), "\"kern\" 1");
}

/// **It is not said in CSS**, and that is deliberate: Stylo 0.8 has neither
/// `font-kerning` nor `font-feature-settings` outside Gecko, so a declaration
/// would be dropped as unknown while reading like a fix. It goes out as the
/// `data-font-features` attribute the vendored `blitz-dom` reads.
#[test]
fn kerning_is_not_emitted_as_a_css_declaration() {
    let css = span_css(&resolved_span(), &FamilyMap::new());
    assert!(!css.contains("font-kerning"), "inert declaration: {css}");
    assert!(!css.contains("font-feature"), "inert declaration: {css}");
}

/// **Whitespace is preserved, not collapsed.** A paragraph reaches this view as
/// one span per resolved run, and CSS `normal` trims each one's edges — so the
/// space ending one run and the space beginning the next both vanished, and
/// `the monospaced words here sits` rendered as `themonospaced words heresits`
/// (ADR-0017 §5.5). `loki-layout` shapes the model's text as it stands; this is
/// the declaration that says so.
#[test]
fn whitespace_is_preserved_so_runs_do_not_lose_the_space_between_them() {
    let css = resolved_para_css(&ResolvedParaProps::default());
    assert!(
        css.contains("white-space: pre-wrap"),
        "a run boundary will eat the space across it: {css}"
    );
}

/// A resolved paragraph emits its alignment, its outside spacing and its
/// indents — the properties that decide where its lines start and end.
#[test]
fn a_resolved_paragraph_emits_its_geometry() {
    let p = ResolvedParaProps {
        space_before: 6.0,
        space_after: 12.0,
        indent_start: 24.0,
        indent_end: 18.0,
        ..ResolvedParaProps::default()
    };
    let css = resolved_para_css(&p);
    assert!(css.contains("margin: 6pt 0 12pt 0"), "{css}");
    assert!(css.contains("padding-inline-start: 24pt"), "{css}");
    assert!(css.contains("padding-inline-end: 18pt"), "{css}");
    assert!(css.contains("text-align:"), "{css}");
}

/// **A hanging indent wins over a first-line one**, which is the model's own
/// order. Both write `text-indent`, so emitting each would let the later one
/// silently replace the earlier.
#[test]
fn a_hanging_indent_beats_a_first_line_one() {
    let both = resolved_para_css(&ResolvedParaProps {
        indent_first_line: 18.0,
        indent_hanging: 36.0,
        ..ResolvedParaProps::default()
    });
    assert_eq!(
        both.matches("text-indent").count(),
        1,
        "two text-indent declarations: {both}"
    );
    assert!(both.contains("text-indent: -36pt"), "{both}");

    // First-line alone is the positive one.
    let first = resolved_para_css(&ResolvedParaProps {
        indent_first_line: 18.0,
        ..ResolvedParaProps::default()
    });
    assert!(first.contains("text-indent: 18pt"), "{first}");

    // Neither: no declaration at all, rather than a zero that would override an
    // ancestor's.
    let none = resolved_para_css(&ResolvedParaProps::default());
    assert!(!none.contains("text-indent"), "{none}");
}

/// **A requested family is emitted as the one that will actually be used.**
///
/// `loki-layout` substitutes a metric-compatible face for a font the host lacks
/// (`FontResources::resolve_font_name`). Emitting the requested name instead
/// leaves Blitz to fall back by its own policy, and the two policies disagree —
/// measured on a screenplay, where the canvas path set monospaced and this one
/// set proportional.
#[test]
fn a_family_is_emitted_substituted_not_as_requested() {
    let s = StyleSpan {
        font_name: Some("Courier Prime".into()),
        ..resolved_span()
    };
    let map: FamilyMap = [("Courier Prime".to_string(), "Liberation Mono".to_string())]
        .into_iter()
        .collect();
    let css = span_css(&s, &map);
    assert!(
        css.contains("font-family: 'Liberation Mono'"),
        "the requested family was emitted instead of the substitute: {css}"
    );
    assert!(
        !css.contains("Courier Prime"),
        "the requested family survived into the CSS: {css}"
    );
}

/// A family with no map entry falls back to the requested name — nothing asked
/// for it during collection, so there is nothing better to say, and dropping the
/// declaration would be worse than an unsubstituted one.
#[test]
fn an_unmapped_family_falls_back_to_the_requested_name() {
    let s = StyleSpan {
        font_name: Some("Some Face".into()),
        ..resolved_span()
    };
    assert!(span_css(&s, &FamilyMap::new()).contains("font-family: 'Some Face'"));
}

/// **A tracked run says so, and an untracked one says nothing.**
///
/// The emission was never the defect — a letter-spaced run beside others lost
/// its tracking inside parley 0.6's shaper (ADR-0017 §5.6, `patches/parley`) —
/// but nothing pinned it either, so a regression here would have looked like the
/// same shaper bug returning.
#[test]
fn letter_spacing_is_emitted_only_when_the_run_carries_it() {
    let tracked = StyleSpan {
        letter_spacing: Some(1.5),
        ..resolved_span()
    };
    assert!(
        span_css(&tracked, &FamilyMap::new()).contains("letter-spacing: 1.5pt"),
        "{}",
        span_css(&tracked, &FamilyMap::new())
    );

    // The inversion: an unset run emits no declaration at all. A `0pt` would
    // override an ancestor's, and CSS `letter-spacing` inherits.
    let css = span_css(&resolved_span(), &FamilyMap::new());
    assert!(!css.contains("letter-spacing"), "{css}");
}
