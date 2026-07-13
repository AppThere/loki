// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 AppThere Loki contributors

//! ODF acid cases — ODT, ODP, ODG, ODS (TEST_PLAN.md §4–§7).
//!
//! ODF has no Microsoft-canonical render: diff against LibreOffice *and* check
//! the ODF round-trip (re-export and compare the targeted XML elements).

use crate::corpus::Severity::{P0, P1, P2};
use crate::corpus::{Format::Odg, Format::Odp, Format::Ods, Format::Odt, TestCase, tc};

/// ODT (14) + ODP (9) + ODG (9) + ODS (10) test cases.
pub(super) const CASES: &[TestCase] = &[
    // ── ODT ──────────────────────────────────────────────────────────────────
    tc(
        "TC-ODT-001",
        Odt,
        P1,
        "Style inheritance (style:parent-style-name)",
    ),
    tc("TC-ODT-002", Odt, P1, "fo:line-height variants"),
    tc("TC-ODT-003", Odt, P0, "Table cell spanning + covered cells"),
    tc("TC-ODT-004", Odt, P1, "List styles + outline"),
    tc("TC-ODT-005", Odt, P1, "Tab stops"),
    tc("TC-ODT-006", Odt, P1, "Sections + columns"),
    tc("TC-ODT-007", Odt, P1, "Frames + wrap"),
    tc("TC-ODT-008", Odt, P1, "Master page / page layout"),
    tc("TC-ODT-009", Odt, P2, "Footnotes/endnotes config"),
    tc("TC-ODT-010", Odt, P2, "Bibliography / fields"),
    tc("TC-ODT-011", Odt, P1, "Change tracking"),
    tc("TC-ODT-012", Odt, P1, "Bidi / writing-mode"),
    tc("TC-ODT-013", Odt, P2, "Drop caps"),
    tc("TC-ODT-014", Odt, P2, "Conditional / hidden text"),
    // ── ODP ──────────────────────────────────────────────────────────────────
    tc("TC-ODP-001", Odp, P1, "Master page inheritance"),
    tc("TC-ODP-002", Odp, P1, "Gradient / bitmap fill"),
    tc("TC-ODP-003", Odp, P2, "Shape effects (shadow)"),
    tc("TC-ODP-004", Odp, P1, "Text autofit"),
    tc("TC-ODP-005", Odp, P1, "Custom shapes"),
    tc("TC-ODP-006", Odp, P1, "Animations"),
    tc("TC-ODP-007", Odp, P2, "Connectors"),
    tc("TC-ODP-008", Odp, P1, "Tables on slides"),
    tc("TC-ODP-009", Odp, P2, "Slide transitions"),
    // ── ODG ──────────────────────────────────────────────────────────────────
    tc("TC-ODG-001", Odg, P1, "Bezier paths"),
    tc("TC-ODG-002", Odg, P1, "Gradient types"),
    tc("TC-ODG-003", Odg, P2, "Hatch + bitmap fill"),
    tc("TC-ODG-004", Odg, P1, "Connectors + glue points"),
    tc("TC-ODG-005", Odg, P2, "Text along path"),
    tc("TC-ODG-006", Odg, P2, "Layers"),
    tc("TC-ODG-007", Odg, P2, "Dimension lines"),
    tc("TC-ODG-008", Odg, P1, "Transforms (rotate/skew/flip)"),
    tc("TC-ODG-009", Odg, P2, "3-D scene"),
    // ── ODS ──────────────────────────────────────────────────────────────────
    tc("TC-ODS-001", Ods, P1, "Data styles (number formats)"),
    tc(
        "TC-ODS-002",
        Ods,
        P0,
        "number:repeated column/row compression",
    ),
    tc("TC-ODS-003", Ods, P0, "Covered cells (merge)"),
    tc("TC-ODS-004", Ods, P1, "Conditional formats"),
    tc("TC-ODS-005", Ods, P0, "ODF formula namespace"),
    tc("TC-ODS-006", Ods, P1, "Cell text rotation"),
    tc("TC-ODS-007", Ods, P1, "Matrix (array) formulas"),
    tc("TC-ODS-008", Ods, P2, "Named ranges/expressions"),
    tc("TC-ODS-009", Ods, P2, "Cell borders + diagonal"),
    tc("TC-ODS-010", Ods, P2, "Frozen panes / split"),
];
