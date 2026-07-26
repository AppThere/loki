// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 AppThere Loki contributors

//! **Spec 09 E0** — how much resident memory is editing residency?
//!
//! Spec 09 gated its phase plan (L9-005) on one experiment: does turning
//! `preserve_for_editing` off actually recover the ~72 bytes per character that
//! `docs/spikes/S09.0-layout-residency-census.md` predicts from struct
//! definitions?
//!
//! It did, for body text — 69.4 B/char against 72 predicted, flat across a 25×
//! document-size change. On **real** documents the rate is a different number
//! per document while the *evictable fraction* stays far narrower. Watch the
//! fraction, not the rate: that is what Spec 09 targets (L9-008, S09.0 §10a).
//!
//! **S9-1 has since landed**, so the numbers this prints are post-change: body
//! text reads ~34.8 B/char editing and ~89.0 total, down from 69.4 and 123.3.
//! The bench now guards that result rather than establishing it. Per L9-013 each
//! later step records its predicted effect on `C` and `P` before implementation
//! and re-runs the sweep to see which coefficient actually moved — S9-1's
//! prediction and outcome are S09.0 §10c and §10d.
//!
//! # Why this is a bench and not a manual RSS comparison
//!
//! Spec 09 r1 proposed diffing RSS across two process runs on real hardware.
//! **Layout is CPU-only** (Parley shaping plus our pagination; the GPU is
//! involved in painting, not layout), so it runs headless, and dhat measures
//! live heap directly — which also dissolves r1's two caveats: the full layout
//! pass is forced by construction, and allocator retention cannot mask what
//! dhat counts. Committed as a bench so it guards S9-1 … S9-5 rather than being
//! spent once (L9-006).
//!
//! # Measurement hygiene
//!
//! Three things this harness does deliberately, all learned from its own bad
//! numbers:
//!
//! - **A process warm-up runs before any measurement**, so shared one-time
//!   costs are not billed to whichever tier happens to run first. Without it the
//!   10-paragraph tier read 411 B/char and looked like a size-dependent floor
//!   artefact; warm, it reads 69.5, indistinguishable from the 250-paragraph
//!   tier. There is no size floor — there was a *first-measurement* artefact.
//! - **A per-document warm-up runs before each row.** The process warm-up
//!   covers only shared costs; a document introducing new fonts pays its own
//!   loading inside its own first measurement. This was worth up to **252×**:
//!   `styles-tinos` read 39,264 B/char cold and 155.7 warm.
//! - **A control re-measures the first document last.** If the warm-ups work,
//!   the two readings agree; if they diverge, the instrument is order-dependent
//!   and every rate in the table is suspect. Printed rather than asserted —
//!   its value is the comparison, not a threshold.
//!
//! Run: `cargo bench -p loki-bench --bench layout_editing_residency`

loki_bench::dhat_global_allocator!();

#[path = "support/mod.rs"]
mod support;

use loki_bench::{AllocStats, measure};
use loki_doc_model::document::Document;
use loki_layout::{FontResources, LayoutMode, LayoutOptions, layout_document};
use std::hint::black_box;

/// Documents below this many characters are dominated by per-document fixed
/// costs, so their per-character rate is not a data point. Reported alongside
/// the rate rather than hidden, so a reader can discount the row themselves.
const RATE_FLOOR_CHARS: usize = 5_000;

/// How far the first and last reading of the same document may differ before
/// the run is declared order-dependent. Tight on purpose: warm, the two agree
/// exactly, so any real drift means a one-time cost is still leaking into a
/// measured region.
const ORDER_DRIFT_TOLERANCE: f64 = 0.05;

fn layout_peak(resources: &mut FontResources, doc: &Document, preserve: bool) -> AllocStats {
    let options = LayoutOptions {
        preserve_for_editing: preserve,
        spell: None,
        ..Default::default()
    };
    // Cold paragraph cache in both conditions, or the second run measures a
    // cache hit and the comparison is meaningless.
    resources.clear_paragraph_cache();
    measure(|| {
        let layout = layout_document(resources, doc, LayoutMode::Paginated, 1.0, &options);
        // Held live across the peak — this is the resident set being measured,
        // not the transient cost of producing it.
        black_box(&layout);
    })
}

/// Lays out `doc` once and discards the result, so any one-time cost it
/// introduces — font loading above all — is paid outside the measured region.
fn warm_doc(resources: &mut FontResources, doc: &Document) {
    let options = LayoutOptions {
        preserve_for_editing: true,
        spell: None,
        ..Default::default()
    };
    black_box(layout_document(
        resources,
        doc,
        LayoutMode::Paginated,
        1.0,
        &options,
    ));
    resources.clear_paragraph_cache();
}

/// Lays out a throwaway document so process-wide one-time costs are paid before
/// the first measurement.
fn warm_up(resources: &mut FontResources) {
    let doc = support::build_doc(4, support::WORDS_PER_PARA);
    let options = LayoutOptions {
        preserve_for_editing: true,
        spell: None,
        ..Default::default()
    };
    black_box(layout_document(
        resources,
        &doc,
        LayoutMode::Paginated,
        1.0,
        &options,
    ));
    resources.clear_paragraph_cache();
}

/// One measured row: both conditions, the retained delta, and the evictable
/// fraction. Returns `(retained_bytes, editing_per_char)`.
fn report_doc(resources: &mut FontResources, label: &str, doc: &Document) -> (i64, f64) {
    let chars = support::char_count(doc);
    // L9-009: a document that yields no characters means the extractor failed,
    // not that the document is empty — the corpus has no empty fixtures. Fail
    // rather than print a tidy "skipped", which is how six fixtures once
    // measured nothing and said so in a way that read as normal.
    assert!(
        chars > 0,
        "{label}: char_count returned 0 — the extractor did not match this \
         document's block or inline shapes, so this row would measure nothing"
    );

    // Per-document warm-up. A process-wide warm-up covers only the *shared*
    // one-time costs; each document that introduces new fonts pays its own
    // loading cost inside whichever of its measurements runs first. Without
    // this, `iris-blueprint` read 176.4 B/char in the corpus loop and 117.5
    // when measured again later in the same run — the same document, 33% apart.
    warm_doc(resources, doc);

    let editing = layout_peak(resources, doc, true);
    let read_only = layout_peak(resources, doc, false);
    let delta = editing.max_bytes as i64 - read_only.max_bytes as i64;
    let per_char = delta as f64 / chars as f64;
    let total_per_char = editing.max_bytes as f64 / chars as f64;
    let evictable = if editing.max_bytes > 0 {
        100.0 * delta as f64 / editing.max_bytes as f64
    } else {
        0.0
    };
    let flag = if chars < RATE_FLOOR_CHARS {
        " (below rate floor)"
    } else {
        ""
    };
    eprintln!(
        "  {label:<24} chars={chars:>7}  editing={per_char:>7.1} B/char  \
         total={total_per_char:>7.1} B/char  evictable={evictable:>5.1}%{flag}",
    );
    (delta, per_char)
}

fn main() {
    support::header("Spec 09 E0 — editing residency: preserve_for_editing on vs off");
    eprintln!(
        "  The evictable % below is a property of each DOCUMENT, not a target for us\n  \
         (L9-008): the engineering goal is what fraction of it we actually reclaim.\n  \
         Rows under {RATE_FLOOR_CHARS} chars are flagged — small documents are fixed-cost heavy."
    );

    let mut resources = FontResources::new();
    // Before anything is measured — see the module docs.
    warm_up(&mut resources);

    let mut worst_delta = 0_i64;
    let mut first_small = 0.0_f64;

    eprintln!("\n  synthetic tiers:");
    for &(name, paras) in support::DOC_TIERS {
        let doc = support::build_doc(paras, support::WORDS_PER_PARA);
        let (delta, per_char) = report_doc(&mut resources, &format!("{name} ({paras}p)"), &doc);
        if name == "small" {
            first_small = per_char;
        }
        worst_delta = worst_delta.max(delta);
    }

    // ── CJK tier (R9-15) ────────────────────────────────────────────────────
    // Every other figure in this program is Latin text. CJK is the sharpest
    // test of whether the per-character model transfers: ~3 bytes per character
    // in UTF-8 against 1, no spaces to break on, and glyph coverage in the
    // thousands rather than under a hundred. The index maps are sized per source
    // *byte*, so S9-2's benefit should be roughly three times larger here — a
    // prediction this row either confirms or kills.
    eprintln!("\n  CJK tier (R9-15 — the per-character model on non-Latin text):");
    {
        let doc = support::build_cjk_doc(120, 6);
        // Coverage sentinel before the rate. A CJK run against Latin-only fonts
        // shapes to a page of tofu that still allocates and still yields a
        // perfectly believable B/char figure — R9-13's failure mode, so it fails
        // rather than prints. Fonts present here: wqy-zenhei, ipafont-gothic.
        let probe = layout_document(
            &mut resources,
            &doc,
            LayoutMode::Paginated,
            1.0,
            &LayoutOptions {
                preserve_for_editing: true,
                spell: None,
                ..Default::default()
            },
        );
        let (glyphs, notdef) = support::glyph_coverage(&probe);
        drop(probe);
        resources.clear_paragraph_cache();
        assert!(
            glyphs > 0 && notdef * 10 < glyphs,
            "CJK tier shaped {glyphs} glyphs of which {notdef} are .notdef — no \
             CJK-capable font resolved, so this row would measure tofu and report \
             it as a per-character rate"
        );
        eprintln!("  (coverage: {glyphs} glyphs, {notdef} .notdef)");
        report_doc(&mut resources, "cjk (120p)", &doc);
    }

    eprintln!("\n  real documents (conformance corpus — six fixtures):");
    let mut corpus_seen = 0_usize;
    let mut iris: Option<Document> = None;
    for &(label, rel) in support::CORPUS {
        match support::load_corpus_doc(rel) {
            Some(doc) => {
                report_doc(&mut resources, label, &doc);
                if label == "iris-blueprint" {
                    iris = Some(doc);
                }
                corpus_seen += 1;
            }
            // Absence is tolerated (the fixture may not be checked out); a
            // document that loads but measures nothing is not — see report_doc.
            None => eprintln!("  {label:<24} unavailable (absent or import failed)"),
        }
    }

    // ── Duplication sweep: decomposing per-placement from content-keyed ─────
    //
    // Repeating a document's blocks makes them byte-identical, so `ParaCache`
    // keys collide: at ×n only 1/n of the paragraphs are distinct content, while
    // `editing_data` still holds an `Arc` per placement. That started as a
    // failed attempt to vary size at constant formatting — it cannot do that,
    // because it changes the cache-hit profile — but the failure measures
    // something no other run in this program does.
    //
    // With `x = unique/total = 1/n`, residency per character is
    // `rate(x) = P + C·x`, where **P is the per-placement cost every copy pays**
    // and **C is the content-keyed cost that deduplicates**. Fitting the line
    // separates them. That bears directly on S9-1: sharing one allocation
    // between `ParaCache` and the editing index removes a copy of the
    // *content-keyed* portion specifically.
    //
    // Product consequence, not just a bench one: residency is per unique
    // paragraph content plus per placement, so documents with repeated
    // boilerplate — form rows, repeated headers, template blocks — deduplicate
    // for free, and a flat B/char figure overstates them.
    //
    // Post-S9-1 this is also the regression guard for the change: `P` is 1.1
    // B/char because the editing index shares the cache's allocation. If a
    // future edit reintroduces a per-placement copy, `P` climbs back toward 39
    // here long before any behavioural test notices — nothing about the output
    // changes when a layout is copied instead of shared.
    if let Some(iris) = iris {
        eprintln!("\n  duplication sweep (x = unique/total; rate = P + C·x):");
        let mut points: Vec<(f64, f64)> = Vec::new();
        for &n in &[1_usize, 2, 5, 10] {
            let doc = if n == 1 {
                iris.clone()
            } else {
                support::repeat_doc(&iris, n)
            };
            let (_, rate) = report_doc(&mut resources, &format!("iris ×{n}"), &doc);
            points.push((1.0 / n as f64, rate));
        }
        let (per_placement, content_keyed) = support::fit_line(&points);
        eprintln!(
            "  fit over {} points: content-keyed C={content_keyed:.1} B/char, \
             per-placement P={per_placement:.1} B/char",
            points.len(),
        );
        eprintln!(
            "  → {:.0}% of editing residency deduplicates across identical paragraphs",
            100.0 * content_keyed / (content_keyed + per_placement).max(1.0),
        );
    }

    // ── Ordering control (L9-011) ───────────────────────────────────────────
    // Re-measures the first subject last. This **asserts** rather than reports:
    // sentinel checks catch an instrument that fails silently, but only a
    // self-consistency check catches one that fails *plausibly*, and plausible
    // wrong answers are the ones that get ratified into specs. 39,264 B/char
    // read exactly like a small-document artefact and was written into Spec 09
    // r3 as established fact; it died only because two of this harness's own
    // rows disagreed by more than any model allowed.
    eprintln!("\n  ordering control (same document, measured last):");
    let small = support::build_doc(support::DOC_TIERS[0].1, support::WORDS_PER_PARA);
    let (_, last_small) = report_doc(&mut resources, "small (control)", &small);
    let drift = (first_small - last_small).abs() / first_small.max(1.0);
    eprintln!(
        "  small tier: first={first_small:.1} B/char  last={last_small:.1} B/char  drift={:.1}%",
        drift * 100.0
    );
    assert!(
        drift <= ORDER_DRIFT_TOLERANCE,
        "E0 is order-dependent: the same document read {first_small:.1} B/char first \
         and {last_small:.1} B/char last ({:.1}% drift, tolerance {:.0}%). Every rate in \
         this run is contaminated by whatever one-time cost the earlier measurement \
         absorbed — fix the warm-up before trusting any figure here.",
        drift * 100.0,
        ORDER_DRIFT_TOLERANCE * 100.0,
    );

    if corpus_seen == 0 {
        eprintln!("\n  note: no corpus documents loaded — synthetic evidence only");
    }

    // The experiment is only meaningful if the two conditions differ at all. A
    // zero delta means `preserve_for_editing` is not the switch the census
    // assumes it is, which is itself the finding — fail loudly.
    assert!(
        worst_delta > 0,
        "E0: preserve_for_editing recovered no memory at any tier — \
         the S09.0 census is wrong about what the flag controls"
    );
}
