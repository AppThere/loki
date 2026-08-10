// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 AppThere Loki contributors

//! Tabbed modal dialog primitives shared by every Loki formatting and
//! publishing dialog.
//!
//! The pieces divide into **pure layout decisions**, which are unit-testable
//! without a window, and the components that paint them:
//!
//! | pure | paints it |
//! | --- | --- |
//! | [`DialogPosture`] · [`DialogWidth`] | [`AtDialogShell`] |
//! | [`DialogTabLayout`] | [`AtDialogTabStrip`] |
//! | [`AtProvenanceKind`] | [`AtProvenanceLine`] |
//!
//! Everything here is **application-agnostic**: no style catalog, no document
//! model, no `fl!()`. Callers pass already-localized strings and map their own
//! provenance type onto [`AtProvenanceKind`] (crate convention — see
//! `appthere-ui/CLAUDE.md`).
//!
//! # The responsive contract
//!
//! Every dialog exposes its **full feature set at all three size classes**;
//! nothing is dropped, only re-laid-out. Expanded is a card with a docked
//! preview rail, Medium narrows and collapses the surplus tabs behind `More ▾`,
//! and Compact is a full-screen sheet driven by a section picker.

mod button;
mod controls;
mod field;
mod posture;
mod provenance;
mod shell;
mod tab_strip;
mod tabs;

pub use button::AtDialogButton;
pub use controls::{AtCheckRow, AtSegmented};
pub use field::{at_control_style, at_field_label_style, AtDialogNotice, AtField, AtNoticeTone};
pub use posture::{DialogPosture, DialogWidth};
pub use provenance::{AtProvenanceKind, AtProvenanceLine};
pub use shell::AtDialogShell;
pub use tab_strip::AtDialogTabStrip;
pub use tabs::DialogTabLayout;
