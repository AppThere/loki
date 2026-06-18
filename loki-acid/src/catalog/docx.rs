// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 AppThere Loki contributors

//! DOCX acid cases (TEST_PLAN.md §1).

use super::{Format::Docx, TestCase, tc};
use crate::severity::Severity::{P0, P1, P2};

/// The 38 DOCX test cases.
pub(super) const CASES: &[TestCase] = &[
    tc(
        "TC-DOCX-001",
        Docx,
        P0,
        "Line spacing lineRule=auto (240ths multiplier)",
    ),
    tc("TC-DOCX-002", Docx, P1, "Line spacing atLeast vs exact"),
    tc("TC-DOCX-003", Docx, P0, "Table vertical merge (vMerge)"),
    tc("TC-DOCX-004", Docx, P1, "Table horizontal merge (gridSpan)"),
    tc("TC-DOCX-005", Docx, P0, "Combined vMerge + gridSpan"),
    tc("TC-DOCX-006", Docx, P1, "Table layout autofit vs fixed"),
    tc("TC-DOCX-007", Docx, P1, "Nested tables"),
    tc("TC-DOCX-008", Docx, P1, "Tab stops + leaders"),
    tc("TC-DOCX-009", Docx, P1, "Decimal tab alignment"),
    tc("TC-DOCX-010", Docx, P0, "Multilevel list numbering"),
    tc("TC-DOCX-011", Docx, P1, "List restart + startOverride"),
    tc(
        "TC-DOCX-012",
        Docx,
        P1,
        "Custom bullet glyphs (Symbol/Wingdings)",
    ),
    tc("TC-DOCX-013", Docx, P1, "Paragraph borders + shading"),
    tc("TC-DOCX-014", Docx, P2, "Character shading / highlight"),
    tc("TC-DOCX-015", Docx, P1, "Drop cap (in-margin / dropped)"),
    tc("TC-DOCX-016", Docx, P1, "Columns + column balancing"),
    tc(
        "TC-DOCX-017",
        Docx,
        P0,
        "Section breaks (page sizes / orientation)",
    ),
    tc("TC-DOCX-018", Docx, P1, "Headers/footers: first/odd/even"),
    tc("TC-DOCX-019", Docx, P1, "Page-number fields + restart"),
    tc("TC-DOCX-020", Docx, P1, "Footnotes + endnotes"),
    tc("TC-DOCX-021", Docx, P0, "Cross-references / bookmarks"),
    tc("TC-DOCX-022", Docx, P2, "Hyperlinks (internal + external)"),
    tc("TC-DOCX-023", Docx, P0, "Floating image wrap modes"),
    tc("TC-DOCX-024", Docx, P1, "Image anchor + position"),
    tc("TC-DOCX-025", Docx, P1, "Text box / shape with text"),
    tc("TC-DOCX-026", Docx, P1, "OMML math equations"),
    tc("TC-DOCX-027", Docx, P0, "Font fallback / substitution"),
    tc("TC-DOCX-028", Docx, P1, "East-Asian + Latin font split"),
    tc("TC-DOCX-029", Docx, P0, "RTL / bidi paragraph"),
    tc(
        "TC-DOCX-030",
        Docx,
        P2,
        "Character spacing / kerning / scale",
    ),
    tc("TC-DOCX-031", Docx, P2, "Small caps vs all caps"),
    tc("TC-DOCX-032", Docx, P1, "Tracked changes display"),
    tc("TC-DOCX-033", Docx, P2, "Comments / ranges"),
    tc("TC-DOCX-034", Docx, P2, "Content controls (SDT)"),
    tc("TC-DOCX-035", Docx, P1, "Watermark"),
    tc("TC-DOCX-036", Docx, P1, "Theme colour resolution"),
    tc(
        "TC-DOCX-037",
        Docx,
        P1,
        "keepNext / keepLines / widowControl",
    ),
    tc(
        "TC-DOCX-038",
        Docx,
        P2,
        "Hanging indent + first-line indent",
    ),
];
