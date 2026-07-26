// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 AppThere Loki contributors

//! **Spec 08 T2.5a** — resident page-texture bytes across zoom × DPI.
//!
//! Phase 2's governing metric, measured before Phase 2 changes anything. The
//! ordering matters: Spec 09's E0 and S9-1 worked because the instrument and
//! the baseline came first, and a budget imposed before a baseline exists has
//! nothing to be compared against.
//!
//! # What it measures
//!
//! A scroll traversal, not a single viewport. The viewport steps through the
//! whole document in half-screen increments; at each stop the mounted set is
//! recomputed with the *production* rule
//! (`appthere_canvas::residency::visible_window`, which `loki_renderer::
//! virtualize` re-exports), departed tiles record a free and arriving tiles
//! record an allocation on the same [`TextureResidency`] counter the real paint
//! source feeds. The reported figure is the **peak** across the traversal,
//! because a budget has to hold at every scroll offset and not merely at the
//! one an author picked.
//!
//! # Why this runs headless
//!
//! Every term is CPU-side: which pages mount is viewport arithmetic, and a page
//! texture is `width × height × 4` for a known format. wgpu is called *after*
//! all of that is decided. Spec 08 R16 was revised on exactly this reasoning —
//! Spec 09 found layout measurable without a device for the same reason, and
//! L08-021 generalises it: name the component that needs hardware before
//! recording anything as blocked on it. What genuinely needs a device is
//! driver-side overhead on top of these bytes; that is the closing gate, and it
//! confirms rather than gates.
//!
//! # Controls (L08-022), and what each one can actually catch
//!
//! - **Process warm-up** and **per-subject warm-up** run a full traversal
//!   outside the measured region. Being honest about these: the measured
//!   quantity is a deterministic integer counter, so unlike Spec 09's dhat
//!   harness — where per-document font loading moved a figure by 252× — these
//!   cannot move the number. They are carried because their *absence* is what
//!   made a Spec 09 figure plausible-but-wrong, and reinstating them later
//!   costs more than keeping them.
//! - **Balance assertion.** Each subject ends with the counter at zero and
//!   `allocs == frees`. This one can fire: a mount/unmount asymmetry leaks
//!   bytes into every later subject, which is the order-dependence that
//!   actually threatens *this* instrument.
//! - **Ordering control.** The first subject is re-measured last and must agree
//!   exactly. Passing establishes that nothing leaked between subjects — a
//!   narrow fact, not a general warrant that the instrument is sound.
//!
//! # Asserted, not printed
//!
//! Spec 09's bench printed 39,264 B/char and it was read as a finding. Every
//! claim this file makes about the tree is an assertion; the table is a
//! by-product of assertions that already passed.
//!
//! Run: `cargo bench -p loki-bench --bench texture_residency`

use std::collections::BTreeSet;

use appthere_canvas::residency::{PageBox, TextureResidency, ViewportSpec, visible_window};

/// Viewport height in CSS px. S0.2 §3's resident-set table assumes 900, so the
/// baseline is directly comparable with the spike it is checking.
const VIEWPORT_H: f64 = 900.0;

/// Zoom levels swept. 0.25 is the clamp floor `document_view.rs` applies; 4.0 is
/// past anything the zoom control offers today and is there to show the law
/// rather than a supported state.
const ZOOMS: &[f64] = &[0.25, 0.5, 1.0, 2.0, 4.0];

/// Device scale factors swept: standard DPI, HiDPI, and the 3× Android phones
/// reach. This is the axis Phase 2 exists to bound, together with zoom.
const SCALES: &[f64] = &[1.0, 2.0, 3.0];

/// Document lengths swept, in pages.
const PAGE_COUNTS: &[usize] = &[10, 100, 500];

/// One traversal's result.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
struct Reading {
    /// Highest resident texture byte total seen at any scroll offset.
    peak_bytes: u64,
    /// Tiles mounted at that peak.
    peak_tiles: usize,
    /// Allocations recorded over the traversal.
    allocs: u64,
}

/// Scrolls a document end to end, maintaining the mounted set exactly as the
/// renderer does, and returns the peak residency seen.
///
/// The mount/unmount order within a step is **free-then-allocate**: departed
/// tiles are released before arriving ones are allocated. That is the
/// steady-state reading. Whether a real frame can transiently hold both sets —
/// Blitz's `release` and `render` are separate callbacks and their relative
/// order within a frame is not established here — is a question for the
/// on-device counter, and would raise the peak by at most one tile.
fn traverse(pages: &[PageBox], zoom: f64, device_scale_factor: f64) -> Reading {
    TextureResidency::reset();
    let vp = ViewportSpec {
        scroll_top_px: 0.0,
        client_height_px: VIEWPORT_H,
        page_gap_px: 24.0,
        zoom,
        device_scale_factor,
    };
    let heights: Vec<f64> = pages.iter().map(|p| p.css_size(zoom).1).collect();
    let doc_height: f64 = heights.iter().map(|h| h + vp.page_gap_px).sum();
    let tile_bytes: Vec<u64> = pages
        .iter()
        .map(|p| p.texture_bytes(zoom, device_scale_factor))
        .collect();

    let mut mounted: BTreeSet<usize> = BTreeSet::new();
    let mut peak_bytes = 0_u64;
    let mut peak_tiles = 0_usize;

    // Half-screen steps, plus a final stop at the very bottom so the last page
    // is always visited however the step size divides the document.
    let step = VIEWPORT_H / 2.0;
    let last_top = (doc_height - VIEWPORT_H).max(0.0);
    let mut top = 0.0_f64;
    loop {
        let want: BTreeSet<usize> = visible_window(&heights, vp.page_gap_px, top, VIEWPORT_H)
            .into_iter()
            .enumerate()
            .filter_map(|(i, visible)| visible.then_some(i))
            .collect();

        for departed in mounted.difference(&want).copied().collect::<Vec<_>>() {
            TextureResidency::record_free(tile_bytes[departed]);
            mounted.remove(&departed);
        }
        for arrived in want.difference(&mounted).copied().collect::<Vec<_>>() {
            TextureResidency::record_alloc(tile_bytes[arrived]);
            mounted.insert(arrived);
        }

        let resident = TextureResidency::resident();
        if resident > peak_bytes {
            peak_bytes = resident;
            peak_tiles = mounted.len();
        }

        if top >= last_top {
            break;
        }
        top = (top + step).min(last_top);
    }

    // Unmount everything, the way closing the document does.
    for page in std::mem::take(&mut mounted) {
        TextureResidency::record_free(tile_bytes[page]);
    }
    let snap = TextureResidency::snapshot();
    assert!(
        snap.is_balanced(),
        "mount/unmount is unbalanced at zoom {zoom} dsf {device_scale_factor}: {snap:?} — \
         every later subject in this run is billed for the residue",
    );
    Reading {
        peak_bytes,
        peak_tiles,
        allocs: snap.allocs,
    }
}

fn doc(page: PageBox, count: usize) -> Vec<PageBox> {
    vec![page; count]
}

fn mib(bytes: u64) -> f64 {
    bytes as f64 / (1024.0 * 1024.0)
}

/// Pays any one-time cost — first allocation of the working vectors, first
/// touch of the counter statics — outside every measured region.
fn warm_up() {
    let _ = traverse(&doc(PageBox::us_letter(), 40), 1.0, 1.0);
    let _ = traverse(&doc(PageBox::a4(), 40), 2.0, 2.0);
    TextureResidency::reset();
}

/// Per-subject warm-up: the same traversal, discarded, so a subject's own first
/// -touch costs are not inside its measurement.
fn warm_subject(pages: &[PageBox], zoom: f64, scale: f64) {
    let _ = traverse(pages, zoom, scale);
    TextureResidency::reset();
}

fn measure(pages: &[PageBox], zoom: f64, scale: f64) -> Reading {
    warm_subject(pages, zoom, scale);
    traverse(pages, zoom, scale)
}

/// The headline table: resident texture bytes across zoom × device scale, on a
/// 500-page US Letter document — the shape S0.2 §3 predicted.
fn zoom_dpi_table() {
    eprintln!("\nResident page-texture bytes — zoom x device scale (US Letter, 500 pages)");
    eprintln!("  viewport {VIEWPORT_H:.0} CSS px, 24 px page gap, peak over a full scroll\n");
    eprintln!(
        "  {:>6}  {:>5}  {:>7}  {:>12}  {:>9}",
        "zoom", "dsf", "tiles", "bytes", "MiB"
    );

    let pages = doc(PageBox::us_letter(), 500);
    for &zoom in ZOOMS {
        for &scale in SCALES {
            let r = measure(&pages, zoom, scale);
            eprintln!(
                "  {:>5.0}%  {:>5.1}  {:>7}  {:>12}  {:>9.1}",
                zoom * 100.0,
                scale,
                r.peak_tiles,
                r.peak_bytes,
                mib(r.peak_bytes),
            );

            // The device scale factor changes tile size and nothing else — the
            // window is computed in CSS px — so residency is exactly quadratic
            // in it. A departure means tile sizing and mounting have become
            // entangled, which is the bug T2.2 could most easily introduce.
            if scale == 2.0 {
                let base = measure(&pages, zoom, 1.0).peak_bytes;
                let ratio = r.peak_bytes as f64 / base as f64;
                assert!(
                    (3.99..4.01).contains(&ratio),
                    "dsf 2.0 must cost exactly 4x dsf 1.0 at zoom {zoom}, got {ratio:.4}x",
                );
            }
        }
    }
}

/// §3.2's load-bearing claim, and this bench's stop condition: resident texture
/// bytes do not scale with document length.
fn document_length_table() {
    eprintln!("\nResident texture bytes vs document length (US Letter)");
    eprintln!(
        "  {:>7}  {:>6}  {:>5}  {:>12}  {:>9}",
        "pages", "zoom", "dsf", "bytes", "MiB"
    );

    for &(zoom, scale) in &[(1.0_f64, 1.0_f64), (2.0, 2.0)] {
        let mut long_readings = Vec::new();
        for &count in PAGE_COUNTS {
            let pages = doc(PageBox::us_letter(), count);
            let r = measure(&pages, zoom, scale);
            eprintln!(
                "  {:>7}  {:>5.0}%  {:>5.1}  {:>12}  {:>9.1}",
                count,
                zoom * 100.0,
                scale,
                r.peak_bytes,
                mib(r.peak_bytes),
            );
            if count >= 100 {
                long_readings.push((count, r.peak_bytes));
            }
        }
        // Any two documents long enough to exceed the window report the same
        // figure. This is why the r1 acceptance criterion (500-page RSS within
        // 20% of 10-page) was withdrawn: texture work cannot move a number that
        // is already equal, and document-length scaling is Spec 09's I-18.
        let all_equal = long_readings.windows(2).all(|w| w[0].1 == w[1].1);
        assert!(
            all_equal,
            "resident texture bytes must be flat in document length, got {long_readings:?}",
        );
    }
}

/// A4 is the default page of a new document, so the baseline records it too.
fn default_page_table() {
    eprintln!("\nResident texture bytes — ISO A4 (the default page), 500 pages");
    eprintln!(
        "  {:>6}  {:>5}  {:>7}  {:>12}  {:>9}",
        "zoom", "dsf", "tiles", "bytes", "MiB"
    );
    let pages = doc(PageBox::a4(), 500);
    for &(zoom, scale) in &[(1.0_f64, 1.0_f64), (1.0, 2.0), (2.0, 2.0), (2.0, 3.0)] {
        let r = measure(&pages, zoom, scale);
        eprintln!(
            "  {:>5.0}%  {:>5.1}  {:>7}  {:>12}  {:>9.1}",
            zoom * 100.0,
            scale,
            r.peak_tiles,
            r.peak_bytes,
            mib(r.peak_bytes),
        );
    }
}

/// The stop condition Spec 08 r13 set for this step: if §3.2 is materially
/// wrong, Phase 2's scope changes and the spec needs revising before any budget
/// is written. Two things have to hold — textures are already windowed, and the
/// 200%-on-HiDPI figure is about 165 MB.
fn check_stop_conditions() {
    let pages = doc(PageBox::us_letter(), 500);
    let worst = measure(&pages, 2.0, 2.0);

    // S0.2 §3 predicts 3 tiles x 55.15 MB = 165.4 MB (decimal MB) at this point.
    let decimal_mb = worst.peak_bytes as f64 / 1_000_000.0;
    assert!(
        (150.0..185.0).contains(&decimal_mb),
        "S0.2 predicts ~165 MB at 200% on HiDPI; measured {decimal_mb:.1} MB. \
         If this is materially off, Phase 2's scope changes — stop and revise \
         the spec before writing a budget.",
    );

    // Windowing: a 500-page document must not mount 500 tiles at any zoom.
    for &zoom in ZOOMS {
        let r = measure(&pages, zoom, 1.0);
        assert!(
            r.peak_tiles < 40,
            "textures must be windowed; {} tiles resident at zoom {zoom}",
            r.peak_tiles,
        );
    }

    eprintln!(
        "\nStop conditions: OK — windowed at every zoom, and 200%/HiDPI reads \
         {decimal_mb:.1} MB against S0.2's 165.4 MB prediction ({} tiles).",
        worst.peak_tiles,
    );
}

fn main() {
    warm_up();

    // The ordering control's first reading (L08-022). Taken before anything
    // else and repeated at the very end.
    let control_doc = doc(PageBox::us_letter(), 500);
    let control_first = measure(&control_doc, 2.0, 2.0);

    zoom_dpi_table();
    document_length_table();
    default_page_table();
    check_stop_conditions();

    let control_last = measure(&control_doc, 2.0, 2.0);
    assert_eq!(
        control_first, control_last,
        "ordering control: the first subject must read identically when \
         re-measured last. A divergence means state leaked between subjects and \
         every figure above is suspect.",
    );
    eprintln!(
        "\nOrdering control: OK — first subject re-read identically last \
         ({} bytes, {} tiles, {} allocations).",
        control_last.peak_bytes, control_last.peak_tiles, control_last.allocs,
    );
}
