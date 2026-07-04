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
//! This is the **M1 skeleton + M2 schema axis**. The schema axis is complete and
//! tested; the round-trip first-divergence differ is implemented; the visual axis
//! is trait + types pending M5. Promotion of `loki-acid`'s catalog / SSIM /
//! golden-discovery into [`golden`] and the per-format model-equality impls are
//! the next passes. A consuming app (Loki Text first) supplies a fixture corpus,
//! a [`roundtrip::NormalizedModel`] impl, an importer/exporter pair, and (later)
//! a CPU render entry point, and gets all three axes — the crate holds no
//! Text-specific assumptions.

#![forbid(unsafe_code)]

pub mod golden;
#[cfg(feature = "doc-model")]
pub mod model;
pub mod roundtrip;
pub mod schema;
#[cfg(feature = "sheet-model")]
pub mod sheet;

pub use roundtrip::{CanonicalEntry, Divergence, NormalizedModel, first_divergence};
pub use schema::xmllint::XmllintValidator;
pub use schema::{SchemaError, SchemaKind, SchemaReport, SchemaValidator, SchemaViolation};
