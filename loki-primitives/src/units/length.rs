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

use super::convert::UnitConversion;
use std::fmt;
use std::ops::{Add, AddAssign, Div, Mul, Neg, Sub, SubAssign};

/// A typed length value in unit `U`.
///
/// The unit type parameter prevents accidentally mixing points with millimeters
/// or pixels with EMUs at compile time. Convert explicitly via `.into_unit::<V>()`.
///
/// Uses `f64` internally for sufficient precision across all unit scales
/// (EMU values can exceed 10^8 for large documents).
#[derive(Debug, Clone, Copy, PartialEq, PartialOrd)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct Length<U> {
    value: f64,
    #[cfg_attr(feature = "serde", serde(skip))]
    _unit: std::marker::PhantomData<U>,
}

impl<U> Length<U> {
    /// Creates a new length value.
    #[must_use]
    pub const fn new(value: f64) -> Self {
        Self {
            value,
            _unit: std::marker::PhantomData,
        }
    }

    /// Extracts the raw scalar value.
    #[must_use]
    pub const fn value(self) -> f64 {
        self.value
    }

    /// Returns `true` if the value is finite.
    #[must_use]
    pub fn is_finite(self) -> bool {
        self.value.is_finite()
    }

    /// Computes the absolute value of the length.
    #[must_use]
    pub fn abs(self) -> Self {
        Self::new(self.value.abs())
    }

    /// Returns the minimum of `self` and `other`.
    #[must_use]
    pub fn min(self, other: Self) -> Self {
        Self::new(self.value.min(other.value))
    }

    /// Returns the maximum of `self` and `other`.
    #[must_use]
    pub fn max(self, other: Self) -> Self {
        Self::new(self.value.max(other.value))
    }

    /// Restricts the value to the range `[min, max]`.
    #[must_use]
    pub fn clamp(self, min: Self, max: Self) -> Self {
        Self::new(self.value.clamp(min.value, max.value))
    }

    /// Returns an additive identity (zero).
    #[must_use]
    pub const fn zero() -> Self {
        Self::new(0.0)
    }

    /// Convert to unit `V`. Requires a known conversion factor between `U` and `V`.
    /// See `UnitConversion` in `units/convert.rs`.
    #[must_use]
    pub fn into_unit<V>(self) -> Length<V>
    where
        U: UnitConversion<V>,
    {
        Length::<V>::new(self.value * U::FACTOR)
    }
}

// Implement Math Traits

impl<U> Add for Length<U> {
    type Output = Self;

    fn add(self, rhs: Self) -> Self::Output {
        Self::new(self.value + rhs.value)
    }
}

impl<U> Sub for Length<U> {
    type Output = Self;

    fn sub(self, rhs: Self) -> Self::Output {
        Self::new(self.value - rhs.value)
    }
}

impl<U> Mul<f64> for Length<U> {
    type Output = Self;

    fn mul(self, rhs: f64) -> Self::Output {
        Self::new(self.value * rhs)
    }
}

impl<U> Div<f64> for Length<U> {
    type Output = Self;

    fn div(self, rhs: f64) -> Self::Output {
        Self::new(self.value / rhs)
    }
}

impl<U> Neg for Length<U> {
    type Output = Self;

    fn neg(self) -> Self::Output {
        Self::new(-self.value)
    }
}

impl<U> AddAssign for Length<U> {
    fn add_assign(&mut self, rhs: Self) {
        self.value += rhs.value;
    }
}

impl<U> SubAssign for Length<U> {
    fn sub_assign(&mut self, rhs: Self) {
        self.value -= rhs.value;
    }
}

// Display Trait
impl<U> fmt::Display for Length<U>
where
    U: DisplayUnit,
{
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}{}", self.value, U::ABBREVIATION)
    }
}

/// Helper trait to associate unit structures with abbreviations for Display.
pub trait DisplayUnit {
    /// Unit string representation.
    const ABBREVIATION: &'static str;
}

use super::unit_types::{Emu, Inch, Mm, Pt, Px, Twip};

impl DisplayUnit for Pt {
    const ABBREVIATION: &'static str = "pt";
}
impl DisplayUnit for Px {
    const ABBREVIATION: &'static str = "px";
}
impl DisplayUnit for Mm {
    const ABBREVIATION: &'static str = "mm";
}
impl DisplayUnit for Inch {
    const ABBREVIATION: &'static str = "in";
}
impl DisplayUnit for Emu {
    const ABBREVIATION: &'static str = "emu";
}
impl DisplayUnit for Twip {
    const ABBREVIATION: &'static str = "twip";
}

#[cfg(test)]
mod tests {
    use super::*;
    use approx::assert_relative_eq;

    #[test]
    fn test_unit_conversions() {
        let pt = Length::<Pt>::new(72.0);
        let inch = pt.into_unit::<Inch>();
        assert_relative_eq!(inch.value, 1.0);

        let mm = Length::<Inch>::new(1.0).into_unit::<Mm>();
        assert_relative_eq!(mm.value, 25.4);

        let emu_to_inch = Length::<Emu>::new(914400.0).into_unit::<Inch>();
        assert_relative_eq!(emu_to_inch.value, 1.0);

        let twip_to_inch = Length::<Twip>::new(1440.0).into_unit::<Inch>();
        assert_relative_eq!(twip_to_inch.value, 1.0);
    }

    #[test]
    fn test_math() {
        let a = Length::<Pt>::new(10.0);
        let b = Length::<Pt>::new(5.0);

        assert_relative_eq!((a + b).value, 15.0);
        assert_relative_eq!((a - b).value, 5.0);
        assert_relative_eq!((a * 2.0).value, 20.0);
        assert_relative_eq!((a / 2.0).value, 5.0);
        assert_relative_eq!((-a).value, -10.0);
    }

    #[test]
    fn test_zero() {
        let x = Length::<Pt>::new(42.0);
        assert_relative_eq!((Length::zero() + x).value, x.value);
    }

    #[cfg(feature = "serde")]
    #[test]
    fn test_serde() {
        let val = Length::<Pt>::new(12.5);
        let serialized = serde_json::to_string(&val).unwrap();
        let deserialized: Length<Pt> = serde_json::from_str(&serialized).unwrap();
        assert_relative_eq!(val.value, deserialized.value);
    }
}
