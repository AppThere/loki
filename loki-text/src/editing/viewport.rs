// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 AppThere Loki contributors

//! The measured editor viewport.
//!
//! **Relocated to `appthere_ui` in Spec 03 M1** so the three suite apps share
//! one viewport + breakpoint classification (Spec 03 §5.3). This module
//! re-exports the shared type so existing `crate::editing::viewport::Viewport`
//! paths keep working; the type, its `centred_origin_x` centring math (Spec 01
//! A-1/A-14), and its tests now live in `appthere_ui::responsive`.

pub use appthere_ui::Viewport;
