// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 AppThere Loki contributors

//! Concurrent-edit, merge-convergence, and pathological-state tests for the
//! `loki-sheet-model` Loro CRDT bridge (audit T-6).
//!
//! The bridge stores each sheet's cells in a `cells` [`LoroMap`] keyed by
//! `"row,col"`, with each cell its own nested map. That structure means
//! concurrent edits to *different* cells merge cleanly (disjoint map keys),
//! while concurrent edits to the *same* cell converge to one deterministic
//! value (a CRDT register). These tests pin down both behaviours and the
//! snapshot/idempotency properties relied on for persistence and sync.

use loki_sheet_model::loro_bridge::{KEY_METADATA, KEY_SHEETS, loro_to_workbook, workbook_to_loro};
use loki_sheet_model::workbook::Workbook;
use loro::{ExportMode, LoroDoc, LoroMap};

/// Exchanges all operations between two replicas so both observe the full op
/// set. After this returns the two docs must converge.
fn sync(a: &LoroDoc, b: &LoroDoc) {
    a.commit();
    b.commit();
    let a_updates = a.export(ExportMode::all_updates()).expect("export a");
    let b_updates = b.export(ExportMode::all_updates()).expect("export b");
    a.import(&b_updates).expect("a imports b");
    b.import(&a_updates).expect("b imports a");
    a.commit();
    b.commit();
}

/// Forks `base` into an independent replica that shares the same history.
fn fork(base: &LoroDoc) -> LoroDoc {
    base.commit();
    base.fork()
}

/// Returns the `cells` map of sheet 0 (always present after `workbook_to_loro`).
fn cells_map(doc: &LoroDoc) -> LoroMap {
    let sheets = doc.get_list(KEY_SHEETS);
    let sheet = sheets
        .get(0)
        .and_then(|v| v.into_container().ok())
        .and_then(|c| c.into_map().ok())
        .expect("sheet 0 map");
    sheet
        .get("cells")
        .and_then(|v| v.into_container().ok())
        .and_then(|c| c.into_map().ok())
        .expect("cells map")
}

/// Sets the value of cell (`row`, `col`) on sheet 0 directly via the CRDT
/// containers — the bridge has no incremental cell-set helper yet.
fn set_cell_value(doc: &LoroDoc, row: u32, col: u32, value: &str) {
    let cm = cells_map(doc);
    let cell = cm
        .insert_container(format!("{row},{col}").as_str(), LoroMap::new())
        .expect("insert cell map");
    cell.insert("value", value).expect("set value");
}

fn cell_value(wb: &Workbook, row: u32, col: u32) -> Option<String> {
    wb.get_sheet(0)?.get_cell(row, col).map(|c| c.value.clone())
}

// ── Round-trip through the public API (external-crate sanity) ─────────────────

#[test]
fn round_trip_via_public_api() {
    let original = Workbook::new();
    let doc = workbook_to_loro(&original).expect("to loro");
    let restored = loro_to_workbook(&doc).expect("from loro");
    assert_eq!(restored, original);
}

// ── Structural merges: different cells / different keys ───────────────────────

#[test]
fn concurrent_edits_to_different_cells_converge() {
    let a = workbook_to_loro(&Workbook::new()).expect("to loro");
    let b = fork(&a);

    set_cell_value(&a, 0, 0, "from-A");
    set_cell_value(&b, 5, 3, "from-B");

    sync(&a, &b);

    assert_eq!(
        a.get_deep_value(),
        b.get_deep_value(),
        "replicas must converge after concurrent edits to disjoint cells"
    );
    let wb = loro_to_workbook(&a).expect("from loro");
    assert_eq!(cell_value(&wb, 0, 0).as_deref(), Some("from-A"));
    assert_eq!(cell_value(&wb, 5, 3).as_deref(), Some("from-B"));
}

#[test]
fn concurrent_metadata_edits_on_different_keys_converge() {
    let a = workbook_to_loro(&Workbook::new()).expect("to loro");
    let b = fork(&a);

    a.get_map(KEY_METADATA)
        .insert("title", "Budget")
        .expect("set title");
    b.get_map(KEY_METADATA)
        .insert("creator", "Ada")
        .expect("set creator");

    sync(&a, &b);

    assert_eq!(a.get_deep_value(), b.get_deep_value());
    let wb = loro_to_workbook(&a).expect("from loro");
    assert_eq!(wb.meta.title.as_deref(), Some("Budget"));
    assert_eq!(wb.meta.creator.as_deref(), Some("Ada"));
}

// ── Register semantics: same cell edited concurrently ─────────────────────────

#[test]
fn concurrent_edit_of_same_cell_is_deterministic() {
    let a = workbook_to_loro(&Workbook::new()).expect("to loro");
    let b = fork(&a);

    set_cell_value(&a, 2, 2, "value-A");
    set_cell_value(&b, 2, 2, "value-B");

    sync(&a, &b);

    assert_eq!(a.get_deep_value(), b.get_deep_value());
    let wb = loro_to_workbook(&a).expect("from loro");
    let v = cell_value(&wb, 2, 2);
    assert!(
        matches!(v.as_deref(), Some("value-A") | Some("value-B")),
        "one writer must win deterministically, got {v:?}"
    );
}

// ── Pathological / edge states ────────────────────────────────────────────────

#[test]
fn snapshot_import_into_fresh_doc_preserves_state() {
    let src = workbook_to_loro(&Workbook::new()).expect("to loro");
    set_cell_value(&src, 1, 1, "persisted");
    src.get_map(KEY_METADATA)
        .insert("title", "Saved")
        .expect("set title");
    src.commit();

    let snapshot = src.export(ExportMode::snapshot()).expect("snapshot");
    let fresh = LoroDoc::new();
    fresh.import(&snapshot).expect("import snapshot");

    assert_eq!(
        src.get_deep_value(),
        fresh.get_deep_value(),
        "a snapshot imported into a fresh doc must reproduce the state exactly"
    );
    let wb = loro_to_workbook(&fresh).expect("from loro");
    assert_eq!(cell_value(&wb, 1, 1).as_deref(), Some("persisted"));
    assert_eq!(wb.meta.title.as_deref(), Some("Saved"));
}

#[test]
fn idempotent_reimport_is_a_noop() {
    let a = workbook_to_loro(&Workbook::new()).expect("to loro");
    let b = fork(&a);
    set_cell_value(&b, 9, 9, "once");
    sync(&a, &b);

    let before = a.get_deep_value();
    let again = b.export(ExportMode::all_updates()).expect("re-export");
    a.import(&again).expect("re-import");
    a.commit();

    assert_eq!(before, a.get_deep_value(), "re-import must be idempotent");
    let wb = loro_to_workbook(&a).expect("from loro");
    assert_eq!(cell_value(&wb, 9, 9).as_deref(), Some("once"));
}
