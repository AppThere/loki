// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 AppThere Loki contributors

use super::{CanonicalEntry, Divergence, NormalizedModel, diff_models, first_divergence};

fn e(path: &str, value: &str) -> CanonicalEntry {
    CanonicalEntry::new(path, value)
}

#[test]
fn equal_forms_have_no_divergence() {
    let a = vec![e("body/p[0]/text", "hi"), e("body/p[0]/bold", "true")];
    let b = a.clone();
    assert_eq!(first_divergence(&a, &b), None);
}

#[test]
fn comparison_is_order_insensitive() {
    let a = vec![e("a", "1"), e("b", "2"), e("c", "3")];
    let b = vec![e("c", "3"), e("a", "1"), e("b", "2")];
    assert_eq!(first_divergence(&a, &b), None);
}

#[test]
fn value_mismatch_reports_both_sides_at_the_path() {
    let a = vec![e("body/p[0]/bold", "true")];
    let b = vec![e("body/p[0]/bold", "false")];
    assert_eq!(
        first_divergence(&a, &b),
        Some(Divergence {
            path: "body/p[0]/bold".to_string(),
            left: Some("true".to_string()),
            right: Some("false".to_string()),
        })
    );
}

#[test]
fn dropped_entry_reports_left_only() {
    // The classic round-trip loss: export drops a run property the import kept.
    let a = vec![e("p[0]/bold", "true"), e("p[0]/italic", "true")];
    let b = vec![e("p[0]/bold", "true")];
    assert_eq!(
        first_divergence(&a, &b),
        Some(Divergence {
            path: "p[0]/italic".to_string(),
            left: Some("true".to_string()),
            right: None,
        })
    );
}

#[test]
fn added_entry_reports_right_only() {
    let a = vec![e("p[0]/bold", "true")];
    let b = vec![e("p[0]/bold", "true"), e("p[0]/strike", "true")];
    assert_eq!(
        first_divergence(&a, &b),
        Some(Divergence {
            path: "p[0]/strike".to_string(),
            left: None,
            right: Some("true".to_string()),
        })
    );
}

#[test]
fn first_divergence_is_the_lowest_path() {
    // Two differ; the lower path (`a`) is reported, not `z`.
    let a = vec![e("a", "1"), e("z", "9")];
    let b = vec![e("a", "2"), e("z", "8")];
    assert_eq!(first_divergence(&a, &b).unwrap().path, "a");
}

struct Doc(Vec<CanonicalEntry>);
impl NormalizedModel for Doc {
    fn canonical(&self) -> Vec<CanonicalEntry> {
        self.0.clone()
    }
}

#[test]
fn diff_models_uses_the_trait() {
    let a = Doc(vec![e("bookmark/id", "_Ref1")]);
    let b = Doc(vec![e("bookmark/id", "_Ref2")]); // mangled bookmark id
    let d = diff_models(&a, &b).expect("ids differ");
    assert_eq!(d.path, "bookmark/id");
    assert_eq!(d.left.as_deref(), Some("_Ref1"));
    assert_eq!(d.right.as_deref(), Some("_Ref2"));
}
