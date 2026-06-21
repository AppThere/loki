// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 AppThere Loki contributors

//! Concurrent-edit, merge-convergence, and pathological-state tests for the
//! `loki-presentation-model` Loro CRDT bridge (audit T-6).
//!
//! The bridge stores presentation metadata as a structural [`LoroMap`] and the
//! slide list as a [`LoroList`] of slide maps; each slide's `drawing` and
//! `placeholders` are coarse-grained JSON-snapshot strings. These tests pin
//! down the resulting merge behaviour:
//!
//! - **Structural keys merge** — concurrent edits to *different* metadata keys
//!   or appends of *different* slides converge with both edits retained.
//! - **Snapshot keys are last-writer-wins registers** — concurrent edits to the
//!   *same* slide snapshot string converge to a single deterministic value, not
//!   a field-level merge. This is the documented MVP limitation; the test exists
//!   to make a future fine-grained bridge a visible, intentional behaviour change.

use loki_presentation_model::loro_bridge::{
    KEY_METADATA, KEY_SLIDES, loro_to_presentation, presentation_to_loro,
};
use loki_presentation_model::presentation::Presentation;
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

/// Appends a minimal slide (just an id) to a replica's slide list, directly via
/// the CRDT containers — the bridge has no incremental slide-insert helper yet.
fn append_slide(doc: &LoroDoc, id: &str) {
    let slides = doc.get_list(KEY_SLIDES);
    let m = slides
        .insert_container(slides.len(), LoroMap::new())
        .expect("insert slide map");
    m.insert("id", id).expect("set id");
}

// ── Round-trip through the public API (external-crate sanity) ─────────────────

#[test]
fn round_trip_via_public_api() {
    let original = Presentation::default();
    let doc = presentation_to_loro(&original).expect("to loro");
    let restored = loro_to_presentation(&doc).expect("from loro");
    assert_eq!(restored, original);
}

// ── Structural merges: different keys / different slides ──────────────────────

#[test]
fn concurrent_metadata_edits_on_different_keys_converge() {
    let a = presentation_to_loro(&Presentation::default()).expect("to loro");
    let b = fork(&a);

    // A sets the title; B sets the author — concurrent, disjoint keys.
    a.get_map(KEY_METADATA)
        .insert("title", "From A")
        .expect("set title");
    b.get_map(KEY_METADATA)
        .insert("author", "From B")
        .expect("set author");

    sync(&a, &b);

    assert_eq!(a.get_deep_value(), b.get_deep_value());
    let p = loro_to_presentation(&a).expect("from loro");
    assert_eq!(p.meta.title.as_deref(), Some("From A"));
    assert_eq!(p.meta.author.as_deref(), Some("From B"));
}

#[test]
fn concurrent_slide_appends_converge_with_both_slides() {
    let a = presentation_to_loro(&Presentation::default()).expect("to loro");
    let b = fork(&a);
    let base_count = loro_to_presentation(&a).unwrap().slide_count();

    append_slide(&a, "slide-from-a");
    append_slide(&b, "slide-from-b");

    sync(&a, &b);

    assert_eq!(
        a.get_deep_value(),
        b.get_deep_value(),
        "replicas must converge after concurrent slide appends"
    );
    let p = loro_to_presentation(&a).expect("from loro");
    assert_eq!(
        p.slide_count(),
        base_count + 2,
        "both concurrently-appended slides must survive the merge"
    );
    let ids: Vec<&str> = p.slides.iter().map(|s| s.id.as_str()).collect();
    assert!(ids.contains(&"slide-from-a"), "lost A's slide: {ids:?}");
    assert!(ids.contains(&"slide-from-b"), "lost B's slide: {ids:?}");
}

// ── Snapshot register semantics (documented MVP limitation) ───────────────────

#[test]
fn concurrent_same_key_metadata_edit_is_last_writer_wins() {
    // Two peers set the *same* metadata key concurrently. A CRDT register
    // converges to ONE deterministic value (ordered by peer id), never a blend.
    let a = presentation_to_loro(&Presentation::default()).expect("to loro");
    let b = fork(&a);

    a.get_map(KEY_METADATA)
        .insert("title", "Title-A")
        .expect("a");
    b.get_map(KEY_METADATA)
        .insert("title", "Title-B")
        .expect("b");

    sync(&a, &b);

    assert_eq!(a.get_deep_value(), b.get_deep_value());
    let title = loro_to_presentation(&a).unwrap().meta.title;
    // Exactly one of the two writes wins — and both replicas agree on which.
    assert!(
        matches!(title.as_deref(), Some("Title-A") | Some("Title-B")),
        "expected one writer to win deterministically, got {title:?}"
    );
}

// ── Pathological / edge states ────────────────────────────────────────────────

#[test]
fn snapshot_import_into_fresh_doc_preserves_state() {
    let src = presentation_to_loro(&Presentation::default()).expect("to loro");
    src.get_map(KEY_METADATA)
        .insert("title", "Persisted")
        .expect("set title");
    append_slide(&src, "extra");
    src.commit();

    let snapshot = src.export(ExportMode::snapshot()).expect("snapshot");
    let fresh = LoroDoc::new();
    fresh.import(&snapshot).expect("import snapshot");

    assert_eq!(
        src.get_deep_value(),
        fresh.get_deep_value(),
        "a snapshot imported into a fresh doc must reproduce the state exactly"
    );
    let p = loro_to_presentation(&fresh).expect("from loro");
    assert_eq!(p.meta.title.as_deref(), Some("Persisted"));
}

#[test]
fn idempotent_reimport_is_a_noop() {
    let a = presentation_to_loro(&Presentation::default()).expect("to loro");
    let b = fork(&a);
    append_slide(&b, "only-once");
    sync(&a, &b);

    let before = a.get_deep_value();
    let again = b.export(ExportMode::all_updates()).expect("re-export");
    a.import(&again).expect("re-import");
    a.commit();

    assert_eq!(before, a.get_deep_value(), "re-import must be idempotent");
    let count = loro_to_presentation(&a)
        .unwrap()
        .slides
        .iter()
        .filter(|s| s.id.as_str() == "only-once")
        .count();
    assert_eq!(count, 1, "the appended slide must not be duplicated");
}
