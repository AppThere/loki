// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 AppThere Loki contributors

//! The ACID test-case catalog — promoted to the shared conformance crate
//! (Spec 02 B-8/B-9); this module re-exports it so existing `loki_acid`
//! callers keep working. New code should use `appthere_conformance::corpus`.

pub use appthere_conformance::corpus::catalog::{
    all_cases, by_feature, cases_for, cases_with_severity,
};
pub use appthere_conformance::corpus::{Format, TestCase};
