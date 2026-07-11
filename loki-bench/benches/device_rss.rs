// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 AppThere Loki contributors

//! Peak-RSS measurement + per-tier budget review (Spec 06 M5 / §9 / §10).
//!
//! Builds and lays out each corpus tier (holding it live) and reads the OS peak
//! RSS (`VmHWM`) — the device-bound memory reality check. It then compares each
//! tier's peak against the committed budgets (`baselines/rss_budgets.txt`) and
//! prints a **review** report — never a CI failure (§11).
//!
//! The mechanism runs headless on Linux (verifiable in the agent), but the
//! *numbers are device-local* and, in the agent, **under-count real devices**
//! (no GPU page textures, different allocator). Kevin must re-run this on the
//! Windows+RTX 3050 / MacBook A16 and `-- --update` the budgets before they are
//! authoritative (audit BM-14). GPU frame-time is the separate `device_frame_time`
//! target (needs a GPU).
//!
//! Run:    `cargo bench -p loki-bench --bench device_rss`
//! Update: `cargo bench -p loki-bench --bench device_rss -- --update`

#[path = "support/mod.rs"]
mod support;

use loki_bench::{BudgetStatus, Budgets, check, current_rss_bytes, headroom_frac, peak_rss_bytes};
use loki_layout::{FontResources, LayoutMode, LayoutOptions, layout_document};
use std::hint::black_box;

const BUDGETS_PATH: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/baselines/rss_budgets.txt");
/// Headroom multiplier over measured peak when calibrating a budget (§9): 50%.
const HEADROOM: f64 = 1.5;

fn mib(bytes: u64) -> f64 {
    bytes as f64 / (1024.0 * 1024.0)
}

/// Builds + lays out each tier (held live) and returns `(tier, peak_rss_bytes)`.
fn measure_tiers() -> Vec<(String, u64)> {
    let Some(base) = peak_rss_bytes() else {
        eprintln!("peak RSS unavailable on this platform (Spec 06 M5 §10: Linux only for now).");
        return Vec::new();
    };
    eprintln!("  process baseline peak RSS: {:.1} MiB", mib(base));

    let mut resources = FontResources::new();
    let options = LayoutOptions {
        preserve_for_editing: true,
        spell: None,
        ..Default::default()
    };
    let mut out = Vec::new();
    for &(name, paras) in support::DOC_TIERS {
        let doc = support::build_doc(paras, support::WORDS_PER_PARA);
        resources.clear_paragraph_cache();
        let layout = layout_document(&mut resources, &doc, LayoutMode::Paginated, 1.0, &options);
        black_box(&layout);
        let peak = peak_rss_bytes().unwrap_or(base);
        let cur = current_rss_bytes().unwrap_or(0);
        eprintln!(
            "  {name:<8} ({paras} paras): peak {:.1} MiB, current {:.1} MiB",
            mib(peak),
            mib(cur),
        );
        out.push((name.to_string(), peak));
    }
    out
}

fn main() {
    eprintln!("\ndevice_rss — peak RSS per corpus tier (device-local; agent under-counts, BM-14):");
    let tiers = measure_tiers();
    if tiers.is_empty() {
        return;
    }

    if std::env::args().any(|a| a == "--update") {
        let pairs: Vec<(String, u64)> = tiers
            .iter()
            .map(|(t, peak)| (t.clone(), (*peak as f64 * HEADROOM) as u64))
            .collect();
        let rendered = Budgets::from_pairs(&pairs).render();
        if let Some(dir) = std::path::Path::new(BUDGETS_PATH).parent() {
            std::fs::create_dir_all(dir).expect("create baselines dir");
        }
        std::fs::write(BUDGETS_PATH, &rendered).expect("write budgets");
        eprintln!("\nbudgets updated ({HEADROOM}× measured peak) → {BUDGETS_PATH}");
        return;
    }

    let Ok(text) = std::fs::read_to_string(BUDGETS_PATH) else {
        eprintln!("\nno committed budgets at {BUDGETS_PATH}; create with `-- --update`.");
        return;
    };
    let budgets = Budgets::parse(&text).expect("parse budgets");
    eprintln!("\nbudget review (target, not gate; §11):");
    for (tier, peak) in &tiers {
        match budgets.get(tier) {
            Some(budget) => {
                let tag = match check(*peak, budget) {
                    BudgetStatus::WithinBudget => "ok",
                    BudgetStatus::OverBudget => "OVER",
                };
                eprintln!(
                    "  [{tag:<4}] {tier:<8} peak {:.1} MiB / budget {:.1} MiB ({:+.0}% headroom)",
                    mib(*peak),
                    mib(budget),
                    headroom_frac(*peak, budget) * 100.0,
                );
            }
            None => eprintln!("  [ ?  ] {tier:<8} no budget set"),
        }
    }
}
