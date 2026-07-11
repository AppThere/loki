// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 AppThere Loki contributors

//! Per-tile font-byte dedup guard (Spec 06 BM-9 / memory-audit F5,
//! deferred-features plan 6.2).
//!
//! `FontDataCache::get_or_insert` clones the full face bytes on a miss.
//! Before 6.2 every page tile owned its own cache, so a face used on M
//! mounted tiles was cloned M times; the fix routes all tiles through the
//! one cache `DocPageSource` already holds. This bench measures both shapes
//! and asserts the shared cache allocates the face bytes **once** — the
//! deterministic tripwire for a shared→per-tile regression.
//!
//! Run: `cargo bench -p loki-bench --bench font_cache_dedup`

loki_bench::dhat_global_allocator!();

use appthere_canvas::FontDataCache;
use loki_bench::measure;
use std::hint::black_box;
use std::sync::Arc;

/// Simulated mounted-tile count (virtualization bounds it to the viewport
/// neighbourhood; ~8 is a generous upper bound).
const TILES: usize = 8;

/// Synthetic face size — 1 MB stands in for a typical post-subset face.
/// (`get_or_insert` clones bytes regardless of font validity; the sfnt strip
/// is a no-op on non-font bytes.)
const FACE_BYTES: usize = 1_000_000;

fn main() {
    let face: Arc<Vec<u8>> = Arc::new(vec![0u8; FACE_BYTES]);

    // Pre-6.2 shape: one cache per tile → M byte-clones of the same face.
    let face_per_tile = Arc::clone(&face);
    let per_tile = measure(move || {
        let mut caches: Vec<FontDataCache> = (0..TILES).map(|_| FontDataCache::new()).collect();
        for cache in &mut caches {
            black_box(cache.get_or_insert(&face_per_tile, 0));
        }
        black_box(&caches);
    });

    // 6.2 shape: every tile paints through the one shared cache → 1 clone.
    let face_shared = Arc::clone(&face);
    let shared = measure(move || {
        let mut cache = FontDataCache::new();
        for _ in 0..TILES {
            black_box(cache.get_or_insert(&face_shared, 0));
        }
        black_box(&cache);
    });

    eprintln!(
        "font_cache_dedup — {TILES} tiles × {FACE_BYTES}B face:\n  \
         per-tile caches: bytes={} allocs={}\n  \
         shared cache:    bytes={} allocs={}",
        per_tile.total_bytes, per_tile.total_blocks, shared.total_bytes, shared.total_blocks,
    );

    // The shared cache must clone the face once, not per tile. Allow slack for
    // map/bookkeeping allocations, but the per-tile shape is ≥ M × face bytes
    // while the shared shape must stay well under 2 faces' worth.
    assert!(
        per_tile.total_bytes >= (TILES * FACE_BYTES) as u64,
        "per-tile baseline no longer clones per tile — update this guard"
    );
    assert!(
        shared.total_bytes < (2 * FACE_BYTES) as u64,
        "shared FontDataCache re-cloned the face bytes per tile (BM-9 regression): \
         {} bytes for {TILES} tiles",
        shared.total_bytes,
    );
}
