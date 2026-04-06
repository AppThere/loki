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

//! Table model types.
//!
//! Modelled on pandoc's table model (pandoc-types ≥ 2.11):
//! `Table Attr Caption [ColSpec] TableHead [TableBody] TableFoot`.
//! TR 29166 §6.2.4 and §7.2.4.

pub mod col;
pub mod core;
pub mod row;

pub use col::{ColAlignment, ColSpec, ColWidth};
pub use core::{Table, TableBody, TableCaption, TableFoot, TableHead};
pub use row::{Cell, CellProps, CellVerticalAlign, Row};
