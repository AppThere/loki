// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 AppThere Loki contributors

//! PPTX acid cases (TEST_PLAN.md §3).
//!
//! A self-generated `acid_pptx.pptx` fixture is now supplied (see
//! `examples/gen_acid_pptx.rs`), so the import/pagination canaries run for the
//! PPTX format. The fixture is written by Loki's own exporter and therefore only
//! covers constructs Loki can already emit — the catalogued cases below that
//! depend on gradients, SmartArt, charts, animations, or grouped-shape child
//! transforms still await a PowerPoint-authored deck and a golden render.

use crate::corpus::Severity::{P0, P1, P2};
use crate::corpus::{Format::Pptx, TestCase, tc};

/// The 29 PPTX test cases.
pub(super) const CASES: &[TestCase] = &[
    tc(
        "TC-PPTX-001",
        Pptx,
        P0,
        "Master -> layout -> slide inheritance",
    ),
    tc("TC-PPTX-002", Pptx, P1, "Theme colour + font scheme"),
    tc("TC-PPTX-003", Pptx, P0, "Text autofit: shrink"),
    tc("TC-PPTX-004", Pptx, P1, "Text autofit: resize shape"),
    tc("TC-PPTX-005", Pptx, P1, "Gradient fills"),
    tc("TC-PPTX-006", Pptx, P1, "Picture / texture / pattern fill"),
    tc(
        "TC-PPTX-007",
        Pptx,
        P1,
        "Shape effects: shadow/glow/reflection",
    ),
    tc("TC-PPTX-008", Pptx, P2, "3-D bevel + rotation"),
    tc("TC-PPTX-009", Pptx, P1, "Custom geometry (freeform)"),
    tc("TC-PPTX-010", Pptx, P2, "Preset geometry adjust handles"),
    tc("TC-PPTX-011", Pptx, P0, "Grouped shapes + child transform"),
    tc("TC-PPTX-012", Pptx, P2, "Connectors"),
    tc("TC-PPTX-013", Pptx, P1, "Tables in slides"),
    tc("TC-PPTX-014", Pptx, P1, "Embedded chart"),
    tc("TC-PPTX-015", Pptx, P1, "SmartArt"),
    tc("TC-PPTX-016", Pptx, P1, "Entrance/exit animations"),
    tc("TC-PPTX-017", Pptx, P2, "Emphasis + motion path"),
    tc("TC-PPTX-018", Pptx, P2, "Slide transitions"),
    tc("TC-PPTX-019", Pptx, P1, "Picture crop + effects"),
    tc("TC-PPTX-020", Pptx, P1, "Bullet formatting"),
    tc("TC-PPTX-021", Pptx, P1, "Line spacing + space before/after"),
    tc("TC-PPTX-022", Pptx, P2, "Text vertical anchor + wrap"),
    tc("TC-PPTX-023", Pptx, P1, "Vertical / rotated text"),
    tc("TC-PPTX-024", Pptx, P2, "Hyperlinks + action buttons"),
    tc(
        "TC-PPTX-025",
        Pptx,
        P2,
        "Slide number / date / footer placeholders",
    ),
    tc("TC-PPTX-026", Pptx, P2, "Header/footer per layout"),
    tc("TC-PPTX-027", Pptx, P1, "Embedded font subset"),
    tc("TC-PPTX-028", Pptx, P2, "Tab stops in text body"),
    tc("TC-PPTX-029", Pptx, P2, "Gradient text / WordArt"),
];
