// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 AppThere Loki contributors

//! Axis 2 — round-trip stability.
//!
//! Importing, exporting, and re-importing a document must not silently lose or
//! mutate semantic content. Comparison is on a **normalized model**, never on
//! bytes: a consumer implements [`NormalizedModel`] for its model type, yielding
//! an order-stable sequence of `(path, value)` [`CanonicalEntry`]s that elides
//! semantically-insignificant differences (element ordering, whitespace,
//! default values) but preserves real content (a dropped run property, a
//! collapsed style, a mangled bookmark id).
//!
//! [`first_divergence`] then reports the **first divergence with a model path**
//! — not just a boolean — so a round-trip failure is diagnosable (Spec 02 §6).
//! The three round-trip *shapes* (native, import-export-import,
//! reference-anchored) and the per-format `NormalizedModel` impls are the
//! consumers' to supply (Spec 02 M3); this module is the shared, format-neutral
//! comparison engine.

/// One entry in a model's canonical form: a stable path and its value.
///
/// Paths should be unique and structural, e.g. `body/para[3]/run[0]/bold`. The
/// comparison is order-insensitive (entries are sorted by path), so the consumer
/// need not emit them in any particular order.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CanonicalEntry {
    /// Structural path identifying this value within the model.
    pub path: String,
    /// The normalized value at `path`.
    pub value: String,
}

impl CanonicalEntry {
    /// Convenience constructor.
    pub fn new(path: impl Into<String>, value: impl Into<String>) -> Self {
        Self {
            path: path.into(),
            value: value.into(),
        }
    }
}

/// A model that can produce an order-stable canonical form for comparison.
pub trait NormalizedModel {
    /// The canonical `(path, value)` entries. Order does not matter
    /// ([`first_divergence`] sorts by path); paths should be unique.
    fn canonical(&self) -> Vec<CanonicalEntry>;
}

/// The first place two canonical forms differ.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Divergence {
    /// The model path at which the forms first diverge.
    pub path: String,
    /// The left (e.g. originally-imported) value at `path`, or `None` if the
    /// left form has no entry there.
    pub left: Option<String>,
    /// The right (re-imported) value at `path`, or `None` if absent there.
    pub right: Option<String>,
}

/// Returns the first divergence between two canonical forms, or `None` if they
/// are equal under the normalized comparison.
///
/// Both forms are sorted by path, then merge-walked: a path present in only one
/// form, or a differing value at a shared path, is the divergence. Reporting the
/// *first* (lowest-path) divergence keeps failures diagnosable.
#[must_use]
pub fn first_divergence(left: &[CanonicalEntry], right: &[CanonicalEntry]) -> Option<Divergence> {
    let mut l: Vec<&CanonicalEntry> = left.iter().collect();
    let mut r: Vec<&CanonicalEntry> = right.iter().collect();
    l.sort_by(|a, b| a.path.cmp(&b.path));
    r.sort_by(|a, b| a.path.cmp(&b.path));

    let (mut i, mut j) = (0usize, 0usize);
    while i < l.len() && j < r.len() {
        match l[i].path.cmp(&r[j].path) {
            std::cmp::Ordering::Less => {
                return Some(Divergence {
                    path: l[i].path.clone(),
                    left: Some(l[i].value.clone()),
                    right: None,
                });
            }
            std::cmp::Ordering::Greater => {
                return Some(Divergence {
                    path: r[j].path.clone(),
                    left: None,
                    right: Some(r[j].value.clone()),
                });
            }
            std::cmp::Ordering::Equal => {
                if l[i].value != r[j].value {
                    return Some(Divergence {
                        path: l[i].path.clone(),
                        left: Some(l[i].value.clone()),
                        right: Some(r[j].value.clone()),
                    });
                }
                i += 1;
                j += 1;
            }
        }
    }
    if let Some(e) = l.get(i) {
        return Some(Divergence {
            path: e.path.clone(),
            left: Some(e.value.clone()),
            right: None,
        });
    }
    if let Some(e) = r.get(j) {
        return Some(Divergence {
            path: e.path.clone(),
            left: None,
            right: Some(e.value.clone()),
        });
    }
    None
}

/// Compares two [`NormalizedModel`]s, returning the first [`Divergence`] or
/// `None` if they round-trip equal.
#[must_use]
pub fn diff_models<M: NormalizedModel>(left: &M, right: &M) -> Option<Divergence> {
    first_divergence(&left.canonical(), &right.canonical())
}

#[cfg(test)]
#[path = "roundtrip_tests.rs"]
mod tests;
