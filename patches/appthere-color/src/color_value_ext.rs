//! Additional color value types and the [`ColorValue`] enum.
//!
//! Contains [`XyzColor`], [`GrayColor`], and the unified [`ColorValue`]
//! discriminated union over all supported color models.

use crate::color_space::ColorSpace;
use crate::color_value::{CmykColor, LabColor, RgbColor};

/// Clamps a value to the range `[min, max]`.
fn clamp(value: f32, min: f32, max: f32) -> f32 {
    if value < min {
        min
    } else if value > max {
        max
    } else {
        value
    }
}

/// A CIE XYZ color with components in `[0.0, 1.0]` (relative, D50).
///
/// # Examples
///
/// ```
/// use appthere_color::XyzColor;
///
/// let color = XyzColor::new(0.5, 0.5, 0.5);
/// assert_eq!(color.x(), 0.5);
/// ```
#[derive(Debug, Clone, Copy, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct XyzColor {
    x: f32,
    y: f32,
    z: f32,
}

impl XyzColor {
    /// Creates a new XYZ color, clamping components to `[0.0, 1.0]`.
    ///
    /// # Examples
    ///
    /// ```
    /// use appthere_color::XyzColor;
    ///
    /// let c = XyzColor::new(1.5, -0.1, 0.5);
    /// assert_eq!(c.x(), 1.0);
    /// assert_eq!(c.y(), 0.0);
    /// ```
    pub fn new(x: f32, y: f32, z: f32) -> Self {
        Self {
            x: clamp(x, 0.0, 1.0),
            y: clamp(y, 0.0, 1.0),
            z: clamp(z, 0.0, 1.0),
        }
    }

    /// Returns the X component.
    ///
    /// # Examples
    ///
    /// ```
    /// use appthere_color::XyzColor;
    /// assert_eq!(XyzColor::new(0.3, 0.0, 0.0).x(), 0.3);
    /// ```
    pub fn x(&self) -> f32 { self.x }

    /// Returns the Y component.
    ///
    /// # Examples
    ///
    /// ```
    /// use appthere_color::XyzColor;
    /// assert_eq!(XyzColor::new(0.0, 0.7, 0.0).y(), 0.7);
    /// ```
    pub fn y(&self) -> f32 { self.y }

    /// Returns the Z component.
    ///
    /// # Examples
    ///
    /// ```
    /// use appthere_color::XyzColor;
    /// assert_eq!(XyzColor::new(0.0, 0.0, 0.9).z(), 0.9);
    /// ```
    pub fn z(&self) -> f32 { self.z }

    /// Returns the components as a 3-element array `[x, y, z]`.
    ///
    /// # Examples
    ///
    /// ```
    /// use appthere_color::XyzColor;
    /// let c = XyzColor::new(0.1, 0.2, 0.3);
    /// assert_eq!(c.to_array(), [0.1, 0.2, 0.3]);
    /// ```
    pub fn to_array(&self) -> [f32; 3] { [self.x, self.y, self.z] }
}

/// A grayscale color with a single component in `[0.0, 1.0]`.
///
/// # Examples
///
/// ```
/// use appthere_color::GrayColor;
///
/// let gray = GrayColor::new(0.5);
/// assert_eq!(gray.value(), 0.5);
/// ```
#[derive(Debug, Clone, Copy, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct GrayColor {
    v: f32,
}

impl GrayColor {
    /// Creates a new grayscale color, clamping the value to `[0.0, 1.0]`.
    ///
    /// # Examples
    ///
    /// ```
    /// use appthere_color::GrayColor;
    ///
    /// let g = GrayColor::new(1.5);
    /// assert_eq!(g.value(), 1.0);
    /// ```
    pub fn new(value: f32) -> Self {
        Self {
            v: clamp(value, 0.0, 1.0),
        }
    }

    /// Returns the grayscale value.
    ///
    /// # Examples
    ///
    /// ```
    /// use appthere_color::GrayColor;
    /// assert_eq!(GrayColor::new(0.42).value(), 0.42);
    /// ```
    pub fn value(&self) -> f32 { self.v }
}

/// A color value in any supported color space.
///
/// This enum provides a unified representation for colors across all
/// supported color models. Use the variant-specific constructors or
/// the [`ColorValue::color_space`] method to determine the model.
///
/// # Examples
///
/// ```
/// use appthere_color::{ColorValue, ColorSpace, RgbColor};
///
/// let color = ColorValue::Rgb(RgbColor::new(1.0, 0.0, 0.0));
/// assert_eq!(color.color_space(), ColorSpace::Rgb);
/// ```
#[derive(Debug, Clone, Copy, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub enum ColorValue {
    /// An RGB color.
    Rgb(RgbColor),
    /// A CMYK color.
    Cmyk(CmykColor),
    /// A CIE L*a*b* color.
    Lab(LabColor),
    /// A CIE XYZ color.
    Xyz(XyzColor),
    /// A grayscale color.
    Gray(GrayColor),
}

impl ColorValue {
    /// Returns the [`ColorSpace`] of this color value.
    ///
    /// # Examples
    ///
    /// ```
    /// use appthere_color::{ColorValue, ColorSpace, CmykColor};
    ///
    /// let color = ColorValue::Cmyk(CmykColor::new(1.0, 0.0, 0.0, 0.0));
    /// assert_eq!(color.color_space(), ColorSpace::Cmyk);
    /// ```
    pub fn color_space(&self) -> ColorSpace {
        match self {
            ColorValue::Rgb(_) => ColorSpace::Rgb,
            ColorValue::Cmyk(_) => ColorSpace::Cmyk,
            ColorValue::Lab(_) => ColorSpace::Lab,
            ColorValue::Xyz(_) => ColorSpace::Xyz,
            ColorValue::Gray(_) => ColorSpace::Gray,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn xyz_clamps_on_construction() {
        let c = XyzColor::new(1.5, -0.1, 0.5);
        assert_eq!(c.x(), 1.0);
        assert_eq!(c.y(), 0.0);
        assert_eq!(c.z(), 0.5);
    }

    #[test]
    fn gray_clamps_on_construction() {
        assert_eq!(GrayColor::new(2.0).value(), 1.0);
        assert_eq!(GrayColor::new(-1.0).value(), 0.0);
    }

    #[test]
    fn color_value_space_detection() {
        let rgb = ColorValue::Rgb(RgbColor::new(0.0, 0.0, 0.0));
        assert_eq!(rgb.color_space(), ColorSpace::Rgb);

        let cmyk = ColorValue::Cmyk(CmykColor::new(0.0, 0.0, 0.0, 0.0));
        assert_eq!(cmyk.color_space(), ColorSpace::Cmyk);

        let lab = ColorValue::Lab(LabColor::new(0.0, 0.0, 0.0));
        assert_eq!(lab.color_space(), ColorSpace::Lab);

        let xyz = ColorValue::Xyz(XyzColor::new(0.0, 0.0, 0.0));
        assert_eq!(xyz.color_space(), ColorSpace::Xyz);

        let gray = ColorValue::Gray(GrayColor::new(0.0));
        assert_eq!(gray.color_space(), ColorSpace::Gray);
    }
}
