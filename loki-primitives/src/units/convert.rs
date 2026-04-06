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

use super::unit_types::*;

/// Conversion factor from unit `Self` to unit `To`.
///
/// Pixel (`Px`) conversions are intentionally excluded, see ADR-0003.
pub trait UnitConversion<To> {
    /// The multiplier to convert a value from `Self` to `To`.
    const FACTOR: f64;
}

macro_rules! impl_conversion {
    ($from:ty, $to:ty, $factor:expr) => {
        impl UnitConversion<$to> for $from {
            const FACTOR: f64 = $factor;
        }
    };
}

// Any unit to itself is 1.0
impl_conversion!(Pt, Pt, 1.0);
impl_conversion!(Px, Px, 1.0);
impl_conversion!(Mm, Mm, 1.0);
impl_conversion!(Inch, Inch, 1.0);
impl_conversion!(Emu, Emu, 1.0);
impl_conversion!(Twip, Twip, 1.0);

// Pt to others
impl_conversion!(Pt, Inch, 1.0 / 72.0);
impl_conversion!(Pt, Mm, 25.4 / 72.0);
impl_conversion!(Pt, Emu, 12700.0); // 914400 / 72
impl_conversion!(Pt, Twip, 20.0); // 1440 / 72

// Inch to others
impl_conversion!(Inch, Pt, 72.0);
impl_conversion!(Inch, Mm, 25.4);
impl_conversion!(Inch, Emu, 914400.0);
impl_conversion!(Inch, Twip, 1440.0);

// Mm to others
impl_conversion!(Mm, Pt, 72.0 / 25.4);
impl_conversion!(Mm, Inch, 1.0 / 25.4);
impl_conversion!(Mm, Emu, 914400.0 / 25.4);
impl_conversion!(Mm, Twip, 1440.0 / 25.4);

// Emu to others
impl_conversion!(Emu, Pt, 72.0 / 914400.0);
impl_conversion!(Emu, Inch, 1.0 / 914400.0);
impl_conversion!(Emu, Mm, 25.4 / 914400.0);
impl_conversion!(Emu, Twip, 1440.0 / 914400.0);

// Twip to others
impl_conversion!(Twip, Pt, 72.0 / 1440.0);
impl_conversion!(Twip, Inch, 1.0 / 1440.0);
impl_conversion!(Twip, Mm, 25.4 / 1440.0);
impl_conversion!(Twip, Emu, 914400.0 / 1440.0);
