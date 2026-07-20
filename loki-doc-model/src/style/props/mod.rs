// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 AppThere Loki contributors

//! Formatting property structs for character and paragraph styling.
//!
//! The property types in this module are derived from TR 29166 §6.2.1
//! (text formatting) and §6.2.2 (paragraph formatting) feature tables.
//! They serve as the raw material for both named styles and direct
//! (inline) formatting overrides.
//!
//! See ADR-0002 (TR 29166 as property authority) and ADR-0003
//! (Option-T for inheritance).

pub mod border;
pub mod char_props;
pub mod drop_cap;
pub mod para_props;
pub mod revision;
pub mod shading;
pub mod tab_stop;

pub use border::{Border, BorderStyle};
pub use char_props::CharProps;
pub use drop_cap::{DropCap, DropCapLength};
pub use para_props::ParaProps;
pub use revision::{RevisionKind, RevisionMark};
pub use shading::{HatchPattern, ShadingPattern};
pub use tab_stop::{TabAlignment, TabLeader, TabStop};
