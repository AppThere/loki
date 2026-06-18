// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 AppThere Loki contributors

//! ACID document-rendering fidelity test harness for the Loki suite.
//!
//! This crate operationalises `TEST_PLAN.md`: a catalog of constructs that
//! office-suite alternatives render differently from the canonical Microsoft
//! 365 (OOXML) / LibreOffice (ODF) render, plus the machinery to diff Loki's
//! output against golden references.
//!
//! # Layers
//!
//! 1. **Catalog** ([`catalog`]) — the machine-readable transcription of every
//!    `TC-*` case with its severity and format.
//! 2. **Fixtures** ([`fixtures`]) — the embedded acid documents and their
//!    import dispatch.
//! 3. **Canaries** ([`pages`]) — page-count and glyph-coverage (tofu) analysis
//!    straight from the layout, no GPU required. These run today.
//! 4. **Pixel diff** ([`diff`], [`golden`]) — SSIM / absolute-difference
//!    metrics and golden-image discovery, ready for the day a headless
//!    rasteriser (or externally produced renders) supplies pixels.
//! 5. **Report** ([`report`]) — per-fixture and aggregate structural reports.
//!
//! # Why no pixels yet
//!
//! Loki's renderer is GPU-backed (Vello / wgpu); a headless rasteriser is not
//! available in every CI environment, and the canonical O365 golden renders are
//! produced manually from Office. The harness therefore asserts the plan's
//! cheap canaries now and exposes the SSIM machinery for the pixel diff once
//! the `goldens/` and `renders/` trees are populated (see `README.md`).

#![forbid(unsafe_code)]
#![warn(missing_docs)]

pub mod catalog;
pub mod diff;
pub mod fixtures;
pub mod golden;
pub mod pages;
pub mod report;
pub mod severity;

pub use catalog::{Format, TestCase, all_cases, cases_for};
pub use fixtures::{Fixture, Imported};
pub use report::{AcidReport, FixtureReport, analyze_fixture, run};
pub use severity::Severity;
