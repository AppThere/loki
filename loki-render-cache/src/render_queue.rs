// Copyright 2026 AppThere Loki contributors
// SPDX-License-Identifier: Apache-2.0

//! Background render-job dispatcher.
//!
//! Only compiled when the `gpu` feature is active.

#![cfg(feature = "gpu")]

use std::sync::{Arc, Mutex, mpsc};

use crate::page_cache::PageCache;
use crate::page_source::PageSource;
use crate::retier::RetierResult;
use crate::texture::downsample_texture;
use crate::{CacheTier, PageIndex};

// ── Job types ─────────────────────────────────────────────────────────────────

enum RenderJob {
    /// Re-render `index` at `tier` from scratch via [`PageSource::render`].
    Rerender { index: PageIndex, tier: CacheTier },
    /// Blit the cached texture for `index` down to `target_tier` resolution.
    Downsample { index: PageIndex, target_tier: CacheTier },
    /// Shut down the worker thread.
    Shutdown,
}

// ── RenderQueue ───────────────────────────────────────────────────────────────

/// Dispatches re-render and downsample jobs to a background thread.
///
/// Create with [`RenderQueue::new`], feed with [`RenderQueue::submit`] after
/// each [`PageCache::retier`] call, and tear down with [`RenderQueue::shutdown`].
pub struct RenderQueue {
    sender: mpsc::Sender<RenderJob>,
    /// Shared cache reference, also needed to look up target tiers in
    /// [`RenderQueue::submit`].
    cache: Arc<Mutex<PageCache>>,
}

impl RenderQueue {
    /// Spawns a background worker thread and returns a [`RenderQueue`] that
    /// sends jobs to it.
    pub fn new(
        cache: Arc<Mutex<PageCache>>,
        source: Arc<dyn PageSource>,
        device: Arc<wgpu::Device>,
        queue: Arc<wgpu::Queue>,
    ) -> Self {
        let (sender, receiver) = mpsc::channel::<RenderJob>();
        let worker_cache = Arc::clone(&cache);
        std::thread::spawn(move || {
            worker_loop(receiver, worker_cache, source, device, queue);
        });
        Self { sender, cache }
    }

    /// Enqueues all jobs in `result`. Called on the main thread immediately
    /// after [`PageCache::retier`].
    ///
    /// For `downsample` pages, the target tier is read from the cache (it was
    /// already updated by `retier`).
    pub fn submit(&self, result: RetierResult) {
        for (index, tier) in result.rerender {
            let _ = self.sender.send(RenderJob::Rerender { index, tier });
        }
        if !result.downsample.is_empty() {
            let guard = self.cache.lock().unwrap_or_else(|e| e.into_inner());
            for index in result.downsample {
                if let Some(page) = guard.get(index) {
                    let _ = self.sender.send(RenderJob::Downsample {
                        index,
                        target_tier: page.tier,
                    });
                }
            }
        }
    }

    /// Sends a shutdown signal and waits for the worker thread to finish.
    pub fn shutdown(self) {
        let _ = self.sender.send(RenderJob::Shutdown);
        // Drop `sender` so the worker unblocks if it's waiting on recv.
        drop(self.sender);
        // `cache` Arc drop happens here; the worker holds its own clone.
    }
}

// ── Worker ────────────────────────────────────────────────────────────────────

fn worker_loop(
    receiver: mpsc::Receiver<RenderJob>,
    cache: Arc<Mutex<PageCache>>,
    source: Arc<dyn PageSource>,
    device: Arc<wgpu::Device>,
    queue: Arc<wgpu::Queue>,
) {
    for job in receiver {
        match job {
            RenderJob::Rerender { index, tier } => {
                match source.render(index, tier.scale_factor(), &device, &queue) {
                    Ok(texture) => {
                        let mut guard = cache.lock().unwrap_or_else(|e| e.into_inner());
                        guard.insert(index, tier, texture);
                    }
                    Err(e) => eprintln!("loki-render-cache: rerender error {index:?}: {e}"),
                }
            }

            RenderJob::Downsample { index, target_tier } => {
                // Remove the page to take ownership of its texture for the blit.
                let entry = {
                    let mut guard = cache.lock().unwrap_or_else(|e| e.into_inner());
                    guard.pages.remove(&index)
                };
                let Some(entry) = entry else { continue };

                // Compute blit scale: target pixel size / current texture size.
                let (orig_w, _) = source.page_size_px(index);
                let src_scale = entry.texture.width as f32 / orig_w as f32;
                let blit_scale = (target_tier.scale_factor() / src_scale).clamp(f32::EPSILON, 1.0);

                let new_texture = downsample_texture(&device, &queue, &entry.texture, blit_scale);
                let mut guard = cache.lock().unwrap_or_else(|e| e.into_inner());
                guard.insert(index, target_tier, new_texture);
            }

            RenderJob::Shutdown => break,
        }
    }
}
