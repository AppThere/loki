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

//! Document color semantics, measurement units, and 2D geometry for the Loki suite.
//!
//! `loki-primitives` is the foundation layer. It provides units, geometry,
//! and abstract document-centric colors, delegating physical color space and
//! transformation math to `appthere-color`.
//!
//! # Examples
//!
//! ## DocumentColor
//! ```
//! use loki_primitives::color::DocumentColor;
//! use loki_primitives::color::ThemeColorSlot;
//!
//! let theme_color = DocumentColor::Theme {
//!     slot: ThemeColorSlot::Accent1,
//!     tint: 0.5,
//! };
//!
//! let cmyk_color = DocumentColor::Cmyk(loki_primitives::color::CmykColor::new(0.0, 1.0, 1.0, 0.0));
//! ```
//!
//! ## Length and Units
//! ```
//! use loki_primitives::units::{Length, Pt, Mm};
//!
//! let points: Length<Pt> = Length::new(72.0);
//! let millimeters: Length<Mm> = points.into_unit::<Mm>();
//! ```
//!
//! ## Geometry
//! ```
//! use loki_primitives::units::Pt;
//! use loki_primitives::geometry::{Point, Size, Rect};
//! use loki_primitives::units::Length;
//!
//! let rect: Rect<Pt> = Rect::new(
//!     Point::new(Length::new(10.0), Length::new(20.0)),
//!     Size::new(Length::new(100.0), Length::new(200.0))
//! );
//! ```

#![warn(unsafe_op_in_unsafe_fn)]

pub mod color;
pub mod geometry;
pub mod units;
