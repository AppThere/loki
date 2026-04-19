// Copyright 2026 AppThere Loki contributors
// SPDX-License-Identifier: Apache-2.0

//! Shared UI components for `loki-text`.
//!
//! * [`toolbar`] — top and bottom toolbar primitives used by the editor shell.
//! * [`wgpu_surface`] — WGPU/Vello document canvas component.
//! * [`document_source`] — `CustomPaintSource` impl for GPU document rendering.

pub mod document_source;
pub mod toolbar;
pub mod wgpu_surface;
