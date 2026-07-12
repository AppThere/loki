// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 AppThere Loki contributors

//! Shared conformance-testing harness for the AppThere Loki suite (Spec 02).
//!
//! Loki claims rendering and round-trip fidelity against Microsoft Office
//! (OOXML) and LibreOffice (ODF). This crate verifies that on three independent
//! axes, each failing for its own reason and pointing at its own bug class:
//!
//! 1. **Schema validation** ([`schema`]) — exported XML is well-formed against
//!    the official OOXML (XSD) / ODF (RELAX NG) schemas. Catches malformed
//!    output regardless of how it renders. **Implemented** (libxml2 backend).
//! 2. **Round-trip stability** ([`roundtrip`]) — import → export → re-import
//!    does not silently lose or mutate semantic content, compared on a
//!    *normalized model*, never on bytes. The first-divergence differ is here;
//!    the per-format `NormalizedModel` impls are the consumers' (Spec 02 M3).
//! 3. **Visual goldens** ([`golden`]) — Loki's render matches a committed golden
//!    within a calibrated perceptual tolerance. Trait + report types here; the
//!    `vello_cpu` candidate render, ΔE/worst-region differ, and calibration are
//!    Spec 02 M5 (and promote `loki-acid`'s SSIM machinery).
//!
//! All three are headless and GPU-free, which lets them run in CI.
//!
//! ## Status (Spec 02 milestones)
//!
//! All three axes are built (M2 schema, M3 round-trip differ + adapters, M5
//! visual differ/calibration/rasterizer), and the shared corpus layer
//! ([`corpus`], Spec 02 B-8/B-9) is promoted from `loki-acid`: the `TC-*`
//! catalog, severity/format vocabulary, the [`corpus::Fixture`] /
//! [`corpus::Consumer`] traits, the on-disk corpus manifest
//! ([`corpus::manifest`], feature × format × axis with per-fixture reference
//! and tolerance records), and golden/candidate discovery
//! ([`golden::discovery`]). A consuming app supplies a fixture corpus, a
//! [`roundtrip::NormalizedModel`] impl, an importer/exporter pair, and a CPU
//! render entry point, and gets all three axes — the crate holds no
//! Text-specific assumptions (`loki-acid` is the first consumer).

#![forbid(unsafe_code)]

pub mod corpus;
pub mod golden;
#[cfg(feature = "doc-model")]
pub mod model;
pub mod raster;
pub mod roundtrip;
pub mod schema;
#[cfg(feature = "sheet-model")]
pub mod sheet;

pub use corpus::{
    Axis, Consumer, Fixture, FixtureMeta, Format, Reference, Severity, TestCase, ToleranceOverride,
};
pub use raster::{CONFORMANCE_DPI, PdfRasterizer, RasterError};
pub use roundtrip::{CanonicalEntry, Divergence, NormalizedModel, first_divergence};
pub use schema::xmllint::XmllintValidator;
pub use schema::{SchemaError, SchemaKind, SchemaReport, SchemaValidator, SchemaViolation};
