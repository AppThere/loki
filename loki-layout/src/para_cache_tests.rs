// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 AppThere Loki contributors

//! Tests for `para_cache` (extracted for the 300-line ceiling).

use crate::color::LayoutColor;
use crate::font::FontResources;
use crate::para::{ResolvedParaProps, StyleSpan, layout_paragraph};

fn resources() -> FontResources {
    let mut r = FontResources::new();
    for p in [
        "/usr/share/fonts/truetype/liberation/LiberationSans-Regular.ttf",
        "/usr/share/fonts/truetype/liberation/LiberationSans-Bold.ttf",
    ] {
        if let Ok(data) = std::fs::read(p) {
            r.register_font(data);
        }
    }
    r
}

fn span(text: &str) -> StyleSpan {
    StyleSpan {
        range: 0..text.len(),
        font_name: None,
        font_size: 12.0,
        bold: false,
        weight: 400,
        italic: false,
        color: LayoutColor::BLACK,
        underline: None,
        strikethrough: None,
        line_height: None,
        vertical_align: None,
        highlight_color: None,
        character_border: None,
        letter_spacing: None,
        font_variant: None,
        word_spacing: None,
        shadow: false,
        emboss: false,
        imprint: false,
        link_url: None,
        math: None,
        scale: None,
        kerning: None,
        baseline_shift: None,
        language: None,
    }
}

fn lay(r: &mut FontResources, text: &str, spans: &[StyleSpan], width: f32) {
    let _ = layout_paragraph(
        r,
        text,
        spans,
        &ResolvedParaProps::default(),
        width,
        1.0,
        true,
    );
}

#[test]
fn identical_inputs_hit_and_match() {
    let mut r = resources();
    let text = "Hello cache world";
    let spans = [span(text)];

    let first = layout_paragraph(
        &mut r,
        text,
        &spans,
        &ResolvedParaProps::default(),
        400.0,
        1.0,
        true,
    );
    assert_eq!(
        r.para_cache.len(),
        1,
        "first call should populate the cache"
    );

    let second = layout_paragraph(
        &mut r,
        text,
        &spans,
        &ResolvedParaProps::default(),
        400.0,
        1.0,
        true,
    );
    // Identical inputs must be a hit (no new entry) and reproduce the layout.
    assert_eq!(
        r.para_cache.len(),
        1,
        "identical call should hit, not insert"
    );
    assert_eq!(first.height, second.height);
    assert_eq!(first.width, second.width);
    assert_eq!(first.items.len(), second.items.len());
}

#[test]
fn changed_inputs_are_misses() {
    let mut r = resources();
    let base = "alpha";

    lay(&mut r, base, &[span(base)], 400.0);
    assert_eq!(r.para_cache.len(), 1);

    // Different text.
    lay(&mut r, "bravo", &[span("bravo")], 400.0);
    assert_eq!(r.para_cache.len(), 2, "different text must miss");

    // Different width, same text/spans.
    lay(&mut r, base, &[span(base)], 200.0);
    assert_eq!(r.para_cache.len(), 3, "different width must miss");

    // Different char property (bold) on the same text.
    let mut bold = span(base);
    bold.bold = true;
    lay(&mut r, base, &[bold], 400.0);
    assert_eq!(r.para_cache.len(), 4, "different style span must miss");

    // Different run language (gap #30): squiggle routing depends on it, so
    // it must participate in the key (covered by the Debug fold).
    let mut tagged = span(base);
    tagged.language = Some("fr-FR".into());
    lay(&mut r, base, &[tagged], 400.0);
    assert_eq!(r.para_cache.len(), 5, "different language must miss");
}

#[test]
fn clear_drops_all_entries() {
    let mut r = resources();
    lay(&mut r, "one", &[span("one")], 400.0);
    lay(&mut r, "two", &[span("two")], 400.0);
    assert_eq!(r.para_cache.len(), 2);

    r.clear_paragraph_cache();
    assert_eq!(r.para_cache.len(), 0, "clear should drop every entry");

    // A subsequent layout repopulates from scratch (miss, not stale hit).
    lay(&mut r, "one", &[span("one")], 400.0);
    assert_eq!(r.para_cache.len(), 1);
}

#[test]
fn preserve_flag_is_part_of_key() {
    let mut r = resources();
    let text = "preserve flag";
    let spans = [span(text)];
    let props = ResolvedParaProps::default();

    let _ = layout_paragraph(&mut r, text, &spans, &props, 400.0, 1.0, true);
    let _ = layout_paragraph(&mut r, text, &spans, &props, 400.0, 1.0, false);
    assert_eq!(
        r.para_cache.len(),
        2,
        "preserve_for_editing must distinguish cache entries"
    );
}

/// Text of the `i`th stable body paragraph in the typing-burst fixtures.
fn stable(i: usize) -> String {
    format!("Stable body paragraph number {i} with enough words to shape a line or two.")
}

/// Lays out the 40-paragraph stable body, as a full layout pass would.
fn lay_stable_body(r: &mut FontResources) {
    for i in 0..40 {
        let t = stable(i);
        lay(r, &t, &[span(&t)], 400.0);
    }
}

/// Types `keystrokes` characters into one paragraph, re-laying the whole
/// document each time — the editor's per-keystroke behaviour.
fn typing_burst(r: &mut FontResources, keystrokes: usize) {
    let mut typed = String::new();
    for step in 0..keystrokes {
        typed.push(if step.is_multiple_of(7) { ' ' } else { 'a' });
        lay_stable_body(r);
        lay(r, &typed, &[span(&typed)], 400.0);
    }
}

/// A typing burst must not grow the cache without bound.
///
/// The key is a hash of the paragraph *text*, so every keystroke mints an entry
/// and the superseded versions stay resident; because each version is longer
/// than the last, the retained bytes grew with the **square** of the burst.
/// Measured before the byte bound: 100 keystrokes → 0.4 MiB, 600 → 8.0 MiB
/// (6× the keystrokes, 21.7× the bytes) against a working set of 41 entries,
/// with the entry cap unable to intervene until 2048.
#[test]
fn a_typing_burst_stays_within_the_byte_bound() {
    let mut r = resources();
    lay_stable_body(&mut r);
    typing_burst(&mut r, 3000);

    let (_, bytes) = r.para_cache_stats();
    let ceiling = 2 * crate::para_cache::GENERATION_BYTE_CAP;
    assert!(
        bytes <= ceiling,
        "cache retained {bytes} bytes, above the two-generation ceiling {ceiling}"
    );
}

/// The bound must be met by evicting **garbage**, not by starving the cache.
///
/// This is the inversion the byte assertion alone cannot make: a `put` that
/// dropped everything, or a `clear()` on every insert, would satisfy the
/// ceiling perfectly while destroying the cache's whole purpose. So after a
/// burst long enough to have forced rotations, a paragraph from the stable body
/// must still be served **without** re-shaping — no new entry appears.
#[test]
fn the_working_set_survives_the_evictions_that_enforce_the_bound() {
    let mut r = resources();
    lay_stable_body(&mut r);
    typing_burst(&mut r, 3000);

    // The only paragraphs that should ever have been shaped are the 40 stable
    // ones (once, at the start) and one new version per keystroke. Anything
    // beyond that is the stable body being re-shaped because eviction threw it
    // away — 40 more for every rotation that did.
    //
    // Two weaker drafts of this assertion were passed by a `clear()`-on-rotate
    // mutant: re-laying the body before measuring made it trivially true, and
    // even measuring residency straight after the burst did not discriminate,
    // because the pass following any clear refills the cache — so it looks
    // populated whenever it is sampled. The re-shape count over the whole burst
    // is the instrument that can actually speak.
    let expected = 40 + 3000;
    assert!(
        r.para_cache.misses <= expected,
        "shaped {} paragraphs where {expected} were unavoidable — the bound is \
         being met by starving the working set rather than by evicting garbage",
        r.para_cache.misses
    );
}

/// The maintained byte counters must equal an independent walk of the entries.
///
/// `stats()` returns running totals rather than walking (the walk is O(glyphs)
/// per entry and ran on every call), so the two derivations are pinned
/// together here: promotion, replacement and rotation each move bytes between
/// the generations, and a counter that drifted would silently move the bound.
#[test]
fn maintained_byte_total_matches_a_full_walk() {
    let mut r = resources();
    lay_stable_body(&mut r);
    // Long enough to force several rotations: the debits and credits only run
    // when there *is* an older generation to promote out of, so a burst that
    // stays under the cap exercises none of the bookkeeping this pins down.
    typing_burst(&mut r, 3000);
    lay_stable_body(&mut r);

    let (_, maintained) = r.para_cache_stats();
    assert_eq!(
        maintained,
        r.para_cache.walked_byte_total(),
        "running byte total drifted from the walked sum"
    );
}

/// The retained Parley layout must be part of the accounting.
///
/// Every editor entry keeps one (`preserve_for_editing`), and it is ~26 B/char
/// against ~18.5 B/char for everything else the accounting walks — so a bound
/// computed without it would be budgeting against a third of the object. The
/// discriminating comparison is the same paragraph cached with and without the
/// retained layout.
#[test]
fn the_retained_parley_layout_is_counted() {
    let text = "A paragraph with enough text to shape several clusters and a line.";

    let mut with = resources();
    let _ = layout_paragraph(
        &mut with,
        text,
        &[span(text)],
        &ResolvedParaProps::default(),
        400.0,
        1.0,
        true,
    );
    let mut without = resources();
    let _ = layout_paragraph(
        &mut without,
        text,
        &[span(text)],
        &ResolvedParaProps::default(),
        400.0,
        1.0,
        false,
    );

    let (_, kept) = with.para_cache_stats();
    let (_, dropped) = without.para_cache_stats();
    assert!(
        kept > dropped,
        "entry retaining a Parley layout ({kept} B) must count more than one \
         without ({dropped} B)"
    );
}

/// The §15/A1 instrumentation: entries and the byte floor must track the
/// cache's real residency — grow on distinct layouts, report non-zero bytes
/// for glyph-bearing entries, and drop to zero on clear (the inversion: a
/// stats() that always reported zero would satisfy a growth-only assertion).
#[test]
fn stats_track_entries_and_drop_to_zero_on_clear() {
    let mut r = resources();
    assert_eq!(r.para_cache_stats(), (0, 0), "empty cache reports zero");

    lay(
        &mut r,
        "some text to shape",
        &[span("some text to shape")],
        400.0,
    );
    lay(&mut r, "other text", &[span("other text")], 400.0);
    let (entries, bytes) = r.para_cache_stats();
    assert_eq!(entries, 2);
    assert!(bytes > 0, "glyph-bearing entries must report bytes");

    r.clear_paragraph_cache();
    assert_eq!(r.para_cache_stats(), (0, 0), "clear zeroes the stats");
}
