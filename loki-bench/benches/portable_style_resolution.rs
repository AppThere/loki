// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 AppThere Loki contributors

//! Portable style-resolution bench (Spec 06 M2 / §6, the headline target).
//!
//! Provenance-aware resolution runs on every inspector open and every dependent
//! recompute (Spec 05). This measures the **allocations** to resolve a leaf
//! style's property when the value lives at the chain root, so each resolve walks
//! the full inheritance chain (worst-case `Inherited`). The sweep is deep chains
//! × many styles, so the depth × count scaling — the §6 super-linear watch — is
//! visible in one table.
//!
//! Run: `cargo bench -p loki-bench --bench portable_style_resolution`

loki_bench::dhat_global_allocator!();

#[path = "support/mod.rs"]
mod support;

use loki_bench::{AllocStats, measure};
use std::hint::black_box;

fn main() {
    support::header(
        "style resolution — allocations to resolve a leaf's alignment (walks the full chain)",
    );
    eprintln!("  each row resolves `chains` leaves, each walking a `depth`-deep chain:");

    let mut worst = AllocStats::default();
    for &depth in support::STYLE_DEPTHS {
        for &chains in support::STYLE_CHAINS {
            let (catalog, leaves) = support::build_style_chains(depth, chains);
            let stats = measure(|| {
                for leaf in &leaves {
                    let resolved = catalog.resolve_para_chain(leaf, |s| s.para_props.alignment);
                    black_box(&resolved);
                }
            });
            support::report_row(&format!("depth={depth:<3} chains={chains:<5}"), stats);
            worst = stats;
        }
    }

    assert!(
        worst.total_bytes > 0,
        "resolution recorded no allocations — is the dhat allocator installed?",
    );
}
