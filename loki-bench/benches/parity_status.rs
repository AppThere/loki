// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 AppThere Loki contributors

//! CPU/GPU parity cadence — version-bump trigger (Spec 06 M6 / §12).
//!
//! Reads the currently pinned Vello version from `Cargo.lock` and compares it to
//! the version the parity check was last **confirmed** against on GPU hardware
//! (`baselines/parity_marker.txt`). Prints whether a parity check is **due** — the
//! mechanical trigger for §12's "on every Vello version bump."
//!
//! This runs headless (it only reads versions), so the *trigger* is verifiable in
//! the agent; the parity *check* it prompts needs a GPU and Spec 02's `vello_cpu`
//! render path (audit BM-3) and runs on-device. After a successful on-device run,
//! `-- --update` records the confirmed version. See docs/adr/spec-06-discipline.md.
//!
//! Run:    `cargo bench -p loki-bench --bench parity_status`
//! Update: `cargo bench -p loki-bench --bench parity_status -- --update` (after an on-device pass)

use loki_bench::{
    ParityStatus, confirmed_version_from_marker, parity_status, render_marker,
    vello_version_from_lock,
};

const LOCK_PATH: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/../Cargo.lock");
const MARKER_PATH: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/baselines/parity_marker.txt");

fn main() {
    let lock = std::fs::read_to_string(LOCK_PATH).expect("read Cargo.lock");
    let Some(current) = vello_version_from_lock(&lock) else {
        eprintln!("no `vello` package in Cargo.lock — nothing to track.");
        return;
    };

    if std::env::args().any(|a| a == "--update") {
        if let Some(dir) = std::path::Path::new(MARKER_PATH).parent() {
            std::fs::create_dir_all(dir).expect("create baselines dir");
        }
        std::fs::write(MARKER_PATH, render_marker(&current)).expect("write marker");
        eprintln!(
            "parity marker updated: confirmed vello {current} → {MARKER_PATH}\n  \
             (only do this AFTER a passing on-device parity run — §12)"
        );
        return;
    }

    let confirmed = std::fs::read_to_string(MARKER_PATH)
        .ok()
        .and_then(|m| confirmed_version_from_marker(&m));

    eprintln!("\nCPU/GPU parity cadence (Spec 06 §12):");
    match parity_status(&current, confirmed.as_deref()) {
        ParityStatus::UpToDate => {
            eprintln!("  [ok]  parity confirmed against pinned vello {current}.");
        }
        ParityStatus::Due { last, current } => {
            eprintln!(
                "  [DUE] vello bumped {last} → {current}. Re-run the CPU/GPU parity\n        \
                 check on GPU hardware; investigate any divergence before trusting the\n        \
                 Spec 02 goldens, then `-- --update`."
            );
        }
        ParityStatus::NeverRun { current } => {
            eprintln!(
                "  [DUE] no confirmed parity run on record for vello {current}. Run the\n        \
                 CPU/GPU parity check on GPU hardware, then `-- --update`."
            );
        }
    }
    eprintln!(
        "  Note: the parity check itself needs a GPU + Spec 02's vello_cpu path (BM-3);\n  \
         this target only detects the version-bump trigger."
    );
}
