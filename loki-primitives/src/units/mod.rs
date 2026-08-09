// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 AppThere Loki contributors

//! Typed length measurement units.

mod convert;
mod length;
mod measurement;
mod unit_types;
mod well_known;

pub use convert::UnitConversion;
pub use length::Length;
pub use measurement::{MeasurementUnit, effective_measurement_unit, resolve_measurement_unit};
pub use unit_types::{Emu, Inch, Mm, Pt, Px, Twip};
pub use well_known::{Emus, Inches, Millimeters, Pixels, Points, Twips};
