// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 AppThere Loki contributors

//! Continuous-memory-tracking tool (Spec 06 M3 / §7 / §11).
//!
//! Collects the curated portable allocation metrics (a representative key per §6
//! target × tier, plus the Arc steady-state guard metric), then diffs them
//! against the committed baseline (`baselines/portable.txt`) and prints deltas
//! for **review** — never a CI failure. `--update` rewrites the baseline
//! intentionally, so git history shows how memory moves over releases.
//!
//! Run:    `cargo bench -p loki-bench --bench baseline`
//! Update: `cargo bench -p loki-bench --bench baseline -- --update`

loki_bench::dhat_global_allocator!();

#[path = "support/mod.rs"]
mod support;

use loki_bench::{
    AllocStats, Baseline, DeltaStatus, Tolerance, any_regressed, diff, measure, render_report,
};
use loki_doc_model::io::{DocumentExport, DocumentImport};
use loki_doc_model::{document_to_loro, loro_to_document};
use loki_layout::{FontResources, LayoutMode, LayoutOptions, layout_document};
use loki_odf::OdtExport;
use loki_ooxml::{DocxExport, DocxImport};
use std::hint::black_box;
use std::io::Cursor;
use std::sync::Arc;

const BASELINE_PATH: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/baselines/portable.txt");

/// Runs the curated tracked workloads once each, returning `(key, stats)` sorted.
fn collect_samples() -> Vec<(String, AllocStats)> {
    let mut out: Vec<(String, AllocStats)> = Vec::new();

    // Style resolution — a representative and a pathological point (§6).
    for &(depth, chains) in &[(16usize, 100usize), (64usize, 1000usize)] {
        let (catalog, leaves) = support::build_style_chains(depth, chains);
        let stats = measure(|| {
            for leaf in &leaves {
                black_box(catalog.resolve_para_chain(leaf, |s| s.para_props.alignment));
            }
        });
        out.push((
            format!("style_resolution/depth{depth}_chains{chains}"),
            stats,
        ));
    }

    // Doc-tier targets: layout, model rebuild, DOCX save/open, ODT export.
    let mut resources = FontResources::new();
    let options = LayoutOptions {
        preserve_for_editing: true,
        spell: None,
        ..Default::default()
    };
    for &(name, paras) in support::DOC_TIERS {
        let doc = support::build_doc(paras, support::WORDS_PER_PARA);

        resources.clear_paragraph_cache();
        let layout = measure(|| {
            black_box(layout_document(
                &mut resources,
                &doc,
                LayoutMode::Paginated,
                1.0,
                &options,
            ));
        });
        out.push((format!("layout/{name}"), layout));

        let loro = document_to_loro(&doc).expect("document_to_loro");
        let model = measure(|| {
            let rebuilt = loro_to_document(&loro).expect("loro_to_document");
            black_box(&rebuilt);
        });
        out.push((format!("model/{name}"), model));

        let mut bytes = Vec::new();
        let save = measure(|| {
            let mut buf = Cursor::new(Vec::new());
            DocxExport::export(&doc, &mut buf, ()).expect("docx export");
            bytes = buf.into_inner();
        });
        out.push((format!("io/{name}_save"), save));

        let open = measure(|| {
            black_box(
                DocxImport::import(Cursor::new(bytes.as_slice()), Default::default())
                    .expect("docx import"),
            );
        });
        out.push((format!("io/{name}_open"), open));

        let odt = measure(|| {
            let mut buf = Cursor::new(Vec::new());
            OdtExport::export(&doc, &mut buf, Default::default()).expect("odt export");
            black_box(buf.into_inner().len());
        });
        out.push((format!("export/{name}_odt"), odt));
    }

    // Arc steady-state guard (audit §4): sharing must not allocate → baseline 0.
    let shared = Arc::new(resources);
    let arc = measure(|| {
        for _ in 0..10_000 {
            black_box(Arc::clone(&shared));
        }
    });
    out.push(("arc/share_font_resources".to_string(), arc));

    out.sort_by(|a, b| a.0.cmp(&b.0));
    out
}

fn main() {
    let samples = collect_samples();

    if std::env::args().any(|a| a == "--update") {
        let rendered = Baseline::from_samples(&samples).render();
        if let Some(dir) = std::path::Path::new(BASELINE_PATH).parent() {
            std::fs::create_dir_all(dir).expect("create baselines dir");
        }
        std::fs::write(BASELINE_PATH, &rendered).expect("write baseline");
        eprintln!("baseline updated: {} keys → {BASELINE_PATH}", samples.len());
        return;
    }

    let Ok(text) = std::fs::read_to_string(BASELINE_PATH) else {
        eprintln!("no committed baseline at {BASELINE_PATH}; create it with `-- --update`.");
        return;
    };
    let base = Baseline::parse(&text).expect("parse committed baseline");
    let deltas = diff(&samples, &base, Tolerance::default());

    eprintln!("\nportable memory baseline diff ({} keys):", deltas.len());
    eprint!("{}", render_report(&deltas));
    let regressed = deltas
        .iter()
        .filter(|d| d.status == DeltaStatus::Regressed)
        .count();
    if any_regressed(&deltas) {
        eprintln!("\n{regressed} key(s) REGRESSED — review (not a CI failure; Spec 06 §11).");
    } else {
        eprintln!("\nno regressions past tolerance.");
    }
}
