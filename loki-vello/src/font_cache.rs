// Copyright 2026 AppThere Loki contributors
// SPDX-License-Identifier: Apache-2.0

//! Re-exports [`appthere_canvas::FontDataCache`] for API stability.
//!
//! `FontDataCache` was moved to `appthere-canvas` for cross-app reuse.
//! COMPAT(loki): callers using `loki_vello::FontDataCache` are unaffected.
pub use appthere_canvas::FontDataCache;
