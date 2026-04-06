// Copyright 2024-2026 AppThere
//
// Licensed under the Apache License, Version 2.0 (the "License");
// you may not use this file except in compliance with the License.
// You may obtain a copy of the License at
//
//     http://www.apache.org/licenses/LICENSE-2.0
//
// Unless required by applicable law or agreed to in writing, software
// distributed under the License is distributed on an "AS IS" BASIS,
// WITHOUT WARRANTIES OR CONDITIONS OF ANY KIND, either express or implied.
// See the License for the specific language governing permissions and
// limitations under the License.

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
pub mod para_props;
pub mod tab_stop;

pub use border::{Border, BorderStyle};
pub use char_props::CharProps;
pub use para_props::ParaProps;
pub use tab_stop::{TabAlignment, TabLeader, TabStop};
