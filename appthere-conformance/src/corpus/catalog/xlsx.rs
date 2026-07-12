// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 AppThere Loki contributors

//! XLSX acid cases (TEST_PLAN.md §2).

use crate::corpus::Severity::{P0, P1, P2};
use crate::corpus::{Format::Xlsx, TestCase, tc};

/// The 30 XLSX test cases.
pub(super) const CASES: &[TestCase] = &[
    tc("TC-XLSX-001", Xlsx, P1, "Custom number format sections"),
    tc("TC-XLSX-002", Xlsx, P1, "Accounting format alignment"),
    tc("TC-XLSX-003", Xlsx, P2, "Fractions & scientific"),
    tc("TC-XLSX-004", Xlsx, P1, "Locale-dependent date/time"),
    tc("TC-XLSX-005", Xlsx, P1, "Conditional format: colour scale"),
    tc("TC-XLSX-006", Xlsx, P1, "Conditional format: data bars"),
    tc("TC-XLSX-007", Xlsx, P1, "Conditional format: icon sets"),
    tc("TC-XLSX-008", Xlsx, P1, "Conditional format: formula rule"),
    tc("TC-XLSX-009", Xlsx, P0, "Dynamic-array spill"),
    tc("TC-XLSX-010", Xlsx, P1, "Legacy CSE array formula"),
    tc(
        "TC-XLSX-011",
        Xlsx,
        P0,
        "Modern functions (XLOOKUP/LET/LAMBDA)",
    ),
    tc("TC-XLSX-012", Xlsx, P1, "Structured table references"),
    tc("TC-XLSX-013", Xlsx, P1, "Merged cells + alignment"),
    tc("TC-XLSX-014", Xlsx, P1, "Text rotation + indent"),
    tc("TC-XLSX-015", Xlsx, P1, "In-cell rich text runs"),
    tc("TC-XLSX-016", Xlsx, P2, "Frozen + split panes"),
    tc("TC-XLSX-017", Xlsx, P1, "Charts: combo + secondary axis"),
    tc("TC-XLSX-018", Xlsx, P2, "Charts: scatter/bubble"),
    tc("TC-XLSX-019", Xlsx, P1, "Sparklines"),
    tc("TC-XLSX-020", Xlsx, P2, "Data validation dropdown"),
    tc("TC-XLSX-021", Xlsx, P2, "Defined names (scoped)"),
    tc("TC-XLSX-022", Xlsx, P1, "Cross-sheet 3-D refs"),
    tc("TC-XLSX-023", Xlsx, P2, "Threaded vs legacy comments"),
    tc("TC-XLSX-024", Xlsx, P1, "Theme + tint cell fill"),
    tc("TC-XLSX-025", Xlsx, P2, "Border precedence"),
    tc("TC-XLSX-026", Xlsx, P2, "Number precision / 1900 leap bug"),
    tc("TC-XLSX-027", Xlsx, P2, "Hidden rows/cols + outline groups"),
    tc("TC-XLSX-028", Xlsx, P2, "Print: areas + scaling + repeat"),
    tc(
        "TC-XLSX-029",
        Xlsx,
        P1,
        "Conditional format priority/stopIfTrue",
    ),
    tc("TC-XLSX-030", Xlsx, P1, "Pivot table render"),
];
