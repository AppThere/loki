// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 AppThere Loki contributors

//! Committed baseline + diff for continuous memory tracking (Spec 06 M3 / §7 / §11).
//!
//! A curated set of portable allocation metrics is rendered to a committed text
//! file (one line per key), diffed against on each run, and updated *only
//! intentionally*. Allocation counts barely move run-to-run, so a real
//! regression stands out well past the jitter tolerance — including the headline
//! case (audit §4): a shared resource cloned by value instead of by `Arc` turns a
//! near-zero metric into a large one, which the diff flags as **Regressed**.
//!
//! This module is pure and headless (no workload deps); the bench that collects
//! the samples and reads/writes the file is `benches/baseline.rs`.

use std::collections::BTreeMap;

use crate::AllocStats;

/// Relative tolerances below which a change is treated as noise (Unchanged).
///
/// Counts are near-deterministic; byte totals drift a little (e.g. DOCX zip
/// ordering), so bytes get more slack. A real regression dwarfs both.
#[derive(Clone, Copy, Debug)]
pub struct Tolerance {
    /// Fractional slack on `total_bytes` (e.g. `0.05` = 5%).
    pub bytes_frac: f64,
    /// Fractional slack on `total_blocks` (allocation count).
    pub blocks_frac: f64,
}

impl Default for Tolerance {
    fn default() -> Self {
        Self {
            bytes_frac: 0.05,
            blocks_frac: 0.02,
        }
    }
}

/// A committed baseline: key → metrics, ordered for a stable, diff-friendly file.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Baseline {
    entries: BTreeMap<String, AllocStats>,
}

impl Baseline {
    /// Builds a baseline from collected `(key, stats)` samples.
    #[must_use]
    pub fn from_samples(samples: &[(String, AllocStats)]) -> Self {
        Self {
            entries: samples.iter().cloned().collect(),
        }
    }

    /// The metrics recorded for `key`, if any.
    #[must_use]
    pub fn get(&self, key: &str) -> Option<AllocStats> {
        self.entries.get(key).copied()
    }

    /// The recorded keys, in sorted order.
    pub fn keys(&self) -> impl Iterator<Item = &String> {
        self.entries.keys()
    }

    /// Number of recorded keys.
    #[must_use]
    pub fn len(&self) -> usize {
        self.entries.len()
    }

    /// Whether the baseline has no entries.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    /// Renders the baseline as a stable, comment-headed, column-aligned file.
    #[must_use]
    pub fn render(&self) -> String {
        let mut out = String::new();
        out.push_str("# loki-bench portable memory baseline (Spec 06 M3).\n");
        out.push_str(
            "# Update intentionally: cargo bench -p loki-bench --bench baseline -- --update\n",
        );
        out.push_str("# key  total_bytes  total_blocks  max_bytes  max_blocks\n");
        for (k, s) in &self.entries {
            out.push_str(&format!(
                "{k:<40} {:>12} {:>10} {:>12} {:>10}\n",
                s.total_bytes, s.total_blocks, s.max_bytes, s.max_blocks,
            ));
        }
        out
    }

    /// Parses the text format, ignoring blank and `#` comment lines.
    ///
    /// # Errors
    /// [`BaselineError`] on a line without five fields or a non-integer metric.
    pub fn parse(text: &str) -> Result<Self, BaselineError> {
        let mut entries = BTreeMap::new();
        for (i, raw) in text.lines().enumerate() {
            let line = raw.trim();
            if line.is_empty() || line.starts_with('#') {
                continue;
            }
            let f: Vec<&str> = line.split_whitespace().collect();
            if f.len() != 5 {
                return Err(BaselineError::Fields {
                    line: i + 1,
                    found: line.to_string(),
                });
            }
            let num = |idx: usize, field: &'static str| -> Result<u64, BaselineError> {
                f[idx].parse::<u64>().map_err(|_| BaselineError::Int {
                    line: i + 1,
                    field,
                    value: f[idx].to_string(),
                })
            };
            entries.insert(
                f[0].to_string(),
                AllocStats {
                    total_bytes: num(1, "total_bytes")?,
                    total_blocks: num(2, "total_blocks")?,
                    max_bytes: num(3, "max_bytes")?,
                    max_blocks: num(4, "max_blocks")?,
                },
            );
        }
        Ok(Self { entries })
    }
}

/// A malformed committed baseline file.
#[derive(Debug, thiserror::Error)]
pub enum BaselineError {
    /// A data line did not have exactly five fields.
    #[error(
        "baseline line {line}: expected `key bytes blocks max_bytes max_blocks`, got {found:?}"
    )]
    Fields {
        /// 1-based line number.
        line: usize,
        /// The offending line.
        found: String,
    },
    /// A metric field was not a valid unsigned integer.
    #[error("baseline line {line}: invalid integer in {field}: {value:?}")]
    Int {
        /// 1-based line number.
        line: usize,
        /// Which column failed to parse.
        field: &'static str,
        /// The offending token.
        value: String,
    },
}

/// How a key's metrics moved relative to the baseline.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum DeltaStatus {
    /// Present now, absent from the baseline.
    New,
    /// Present in the baseline, absent now.
    Removed,
    /// Grew beyond tolerance — the flag for review.
    Regressed,
    /// Shrank beyond tolerance.
    Improved,
    /// Within tolerance either way.
    Unchanged,
}

/// One key's comparison between the current run and the baseline.
pub struct Delta {
    /// The metric key.
    pub key: String,
    /// The movement classification.
    pub status: DeltaStatus,
    /// The baseline metrics, if the key was present there.
    pub baseline: Option<AllocStats>,
    /// The current metrics, if the key was present now.
    pub current: Option<AllocStats>,
    /// Relative change in `total_bytes` (`+INF` when the baseline was zero).
    pub bytes_pct: f64,
    /// Relative change in `total_blocks`.
    pub blocks_pct: f64,
}

/// Diffs the current samples against the baseline, classifying each key.
#[must_use]
pub fn diff(current: &[(String, AllocStats)], baseline: &Baseline, tol: Tolerance) -> Vec<Delta> {
    let cur_keys: BTreeMap<&str, AllocStats> =
        current.iter().map(|(k, s)| (k.as_str(), *s)).collect();
    let mut deltas = Vec::new();

    for (k, s) in current {
        match baseline.get(k) {
            Some(base) => {
                let bytes_pct = rel(s.total_bytes, base.total_bytes);
                let blocks_pct = rel(s.total_blocks, base.total_blocks);
                deltas.push(Delta {
                    key: k.clone(),
                    status: classify(bytes_pct, blocks_pct, tol),
                    baseline: Some(base),
                    current: Some(*s),
                    bytes_pct,
                    blocks_pct,
                });
            }
            None => deltas.push(Delta {
                key: k.clone(),
                status: DeltaStatus::New,
                baseline: None,
                current: Some(*s),
                bytes_pct: 0.0,
                blocks_pct: 0.0,
            }),
        }
    }

    for k in baseline.keys() {
        if !cur_keys.contains_key(k.as_str()) {
            deltas.push(Delta {
                key: k.clone(),
                status: DeltaStatus::Removed,
                baseline: baseline.get(k),
                current: None,
                bytes_pct: 0.0,
                blocks_pct: 0.0,
            });
        }
    }

    deltas.sort_by(|a, b| a.key.cmp(&b.key));
    deltas
}

/// `true` when any key regressed — the signal a reviewer acts on (never a CI
/// failure; §11).
#[must_use]
pub fn any_regressed(deltas: &[Delta]) -> bool {
    deltas.iter().any(|d| d.status == DeltaStatus::Regressed)
}

/// Renders a one-line-per-key review report.
#[must_use]
pub fn render_report(deltas: &[Delta]) -> String {
    let mut out = String::new();
    for d in deltas {
        let tag = match d.status {
            DeltaStatus::New => "NEW",
            DeltaStatus::Removed => "REMOVED",
            DeltaStatus::Regressed => "REGRESSED",
            DeltaStatus::Improved => "improved",
            DeltaStatus::Unchanged => "ok",
        };
        out.push_str(&format!(
            "  [{tag:<9}] {:<40} bytes {}  allocs {}\n",
            d.key,
            fmt_pct(d.bytes_pct),
            fmt_pct(d.blocks_pct),
        ));
    }
    out
}

/// Relative change `(cur - base) / base`, with `+INF` when the baseline was zero
/// and something now allocates — the Arc-share-regressed case.
fn rel(cur: u64, base: u64) -> f64 {
    if base == 0 {
        if cur == 0 { 0.0 } else { f64::INFINITY }
    } else {
        (cur as f64 - base as f64) / base as f64
    }
}

fn classify(bytes_pct: f64, blocks_pct: f64, tol: Tolerance) -> DeltaStatus {
    if bytes_pct > tol.bytes_frac || blocks_pct > tol.blocks_frac {
        DeltaStatus::Regressed
    } else if bytes_pct < -tol.bytes_frac || blocks_pct < -tol.blocks_frac {
        DeltaStatus::Improved
    } else {
        DeltaStatus::Unchanged
    }
}

fn fmt_pct(p: f64) -> String {
    if p.is_finite() {
        format!("{:+.1}%", p * 100.0)
    } else {
        "  new-alloc".to_string()
    }
}

#[cfg(test)]
#[path = "baseline_tests.rs"]
mod tests;
