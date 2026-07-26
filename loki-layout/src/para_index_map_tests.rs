// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 AppThere Loki contributors

//! Tests for [`super::ByteIndexMap`] (extracted for the 300-line ceiling).
//!
//! The property that matters is that the compact form is *indistinguishable*
//! from the `Vec<usize>` it replaces at every offset, including past the end —
//! S9-2 is a representation change and must not be a behaviour change.

use super::ByteIndexMap;

/// The reference behaviour: what the call sites did by hand with a `Vec`.
fn reference(v: &[usize], i: usize) -> usize {
    v.get(i)
        .copied()
        .unwrap_or_else(|| v.last().copied().unwrap_or(0))
}

fn agrees_with_reference(indices: &[usize]) {
    let map = ByteIndexMap::from_indices(indices);
    assert_eq!(map.len(), indices.len());
    assert_eq!(map.last(), indices.last().copied());
    // Probe past the end too: Parley's cursor lands on the end sentinel, and
    // one past it is the case the clamp exists for.
    for i in 0..indices.len() + 3 {
        assert_eq!(
            map.get_clamped(i),
            reference(indices, i),
            "offset {i} diverged for {indices:?}"
        );
        assert_eq!(map.get(i), indices.get(i).copied(), "get({i}) diverged");
    }
}

#[test]
fn identity_input_compacts_to_identity() {
    let indices: Vec<usize> = (0..64).collect();
    assert_eq!(
        ByteIndexMap::from_indices(&indices),
        ByteIndexMap::Identity { len: 64 },
        "a map with nothing removed must not allocate an array"
    );
    agrees_with_reference(&indices);
}

#[test]
fn a_single_removed_byte_forces_the_mapped_form() {
    // One character dropped at offset 3: offsets past it shift down by one.
    let indices = vec![0, 1, 2, 3, 3, 4, 5];
    assert!(matches!(
        ByteIndexMap::from_indices(&indices),
        ByteIndexMap::Mapped(_)
    ));
    agrees_with_reference(&indices);
}

#[test]
fn empty_and_singleton_maps_behave() {
    let empty = ByteIndexMap::from_indices(&[]);
    assert!(empty.is_empty());
    assert_eq!(empty.last(), None);
    assert_eq!(empty.get(0), None);
    // The documented fallback for an empty map is 0, matching the old `Vec`
    // call sites — an evicted-style silent wrong answer is not wanted here.
    assert_eq!(empty.get_clamped(0), 0);
    agrees_with_reference(&[]);

    // The zero-height synthetic paragraph built by the keep-with-next chain.
    agrees_with_reference(&[0]);
}

#[test]
fn a_non_monotonic_map_still_round_trips() {
    // Not produced by the cleaner, but the type must not silently reorder or
    // dedupe: it is a representation, not a model of what the cleaner can emit.
    agrees_with_reference(&[5, 0, 9, 2]);
}

#[test]
fn the_mapped_form_is_exactly_sized() {
    let indices = vec![0, 0, 1, 2, 2, 3];
    match ByteIndexMap::from_indices(&indices) {
        ByteIndexMap::Mapped(v) => assert_eq!(
            v.len(),
            indices.len(),
            "Box<[u32]> must be exact — the whole point is not carrying slack"
        ),
        ByteIndexMap::Identity { .. } => panic!("expected the mapped form"),
    }
}
