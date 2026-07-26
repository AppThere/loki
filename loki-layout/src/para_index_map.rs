// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 AppThere Loki contributors

//! Compact byte-index maps between a paragraph's original and cleaned text.
//!
//! Spec 09 S9-2. `clean_text_and_spans` removes characters Parley must not see
//! (control characters, the BOM, tabs) and produces two maps so editor offsets
//! survive the removal. Held as `Vec<usize>` they were one 8-byte entry per
//! source byte each — **~16 B/char for ASCII**, about a fifth of a paragraph's
//! editing residency, and plain index arithmetic rather than shaping data
//! (`docs/spikes/S09.0-layout-residency-census.md` §2.3).
//!
//! Two observations shrink that:
//!
//! 1. **Offsets are bounded by the paragraph's own length**, so `u32` suffices
//!    and halves the cost.
//! 2. **Most paragraphs remove nothing at all**, and for those the map *is* the
//!    identity function — representable in one `usize` instead of an array.
//!
//! Built as `Vec<usize>` during layout (the construction logic is unchanged and
//! needs random-access mutation for the drop-cap rebase) and compacted once, at
//! the point the map is stored, by [`ByteIndexMap::from_indices`].

/// A monotonic map from one byte-offset space to another, stored compactly.
///
/// Indexing is clamping rather than panicking — see [`Self::get_clamped`] — to
/// match how every caller already used the `Vec`: `get(i)` falling back to the
/// last entry. Offsets past the end are a normal consequence of Parley's cursor
/// landing on the end sentinel, not an error.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ByteIndexMap {
    /// Nothing was removed, so entry `i` is `i` for all `i < len`.
    ///
    /// `len` counts entries, i.e. `text.len() + 1` including the end sentinel.
    Identity {
        /// Number of entries this map covers.
        len: usize,
    },
    /// An explicit mapping, one entry per source byte plus the end sentinel.
    ///
    /// `Box<[u32]>` rather than `Vec<u32>`: the map is built once and never
    /// grown, so the capacity word is dead weight and an exact allocation is
    /// guaranteed rather than merely likely.
    Mapped(Box<[u32]>),
}

impl ByteIndexMap {
    /// Compacts a freshly-built index vector.
    ///
    /// Detects the identity case in one pass — cheap next to shaping, and paid
    /// only on a paragraph-cache miss.
    ///
    /// Entries are narrowed to `u32`, saturating at [`u32::MAX`]. A paragraph
    /// long enough to overflow that is 4 GiB of text in a single block; it would
    /// exhaust memory during shaping long before reaching here, so saturation is
    /// a formality rather than a behaviour anyone can observe. It is a clamp and
    /// not a panic because this is library code.
    pub fn from_indices(indices: &[usize]) -> Self {
        if indices.iter().enumerate().all(|(i, &v)| i == v) {
            return Self::Identity { len: indices.len() };
        }
        Self::Mapped(
            indices
                .iter()
                .map(|&v| u32::try_from(v).unwrap_or(u32::MAX))
                .collect(),
        )
    }

    /// Number of entries, including the end sentinel.
    pub fn len(&self) -> usize {
        match self {
            Self::Identity { len } => *len,
            Self::Mapped(v) => v.len(),
        }
    }

    /// Whether the map covers no offsets at all.
    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }

    /// The entry at `i`, or `None` if `i` is past the end.
    pub fn get(&self, i: usize) -> Option<usize> {
        match self {
            Self::Identity { len } => (i < *len).then_some(i),
            Self::Mapped(v) => v.get(i).map(|&x| x as usize),
        }
    }

    /// The last entry, or `None` for an empty map.
    pub fn last(&self) -> Option<usize> {
        match self {
            Self::Identity { len } => len.checked_sub(1),
            Self::Mapped(v) => v.last().map(|&x| x as usize),
        }
    }

    /// The entry at `i`, clamped to the last entry, or `0` for an empty map.
    ///
    /// This is what every call site did by hand with the `Vec`; naming it once
    /// keeps the clamping policy in one place rather than restated at each use.
    pub fn get_clamped(&self, i: usize) -> usize {
        self.get(i)
            .unwrap_or_else(|| self.last().unwrap_or_default())
    }
}

#[cfg(test)]
#[path = "para_index_map_tests.rs"]
mod tests;
