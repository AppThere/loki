//! Color space identification for ICC profile matching and transform layout.
//!
//! Provides the [`ColorSpace`] enum which identifies the color model of a
//! profile or color value, and maps to `moxcms::DataColorSpace` for transform
//! layout selection.

use crate::error::{ColorError, ColorResult};

/// Identifies a color model for profile matching and transform setup.
///
/// Each variant corresponds to a specific ICC color model. The library uses
/// this enum to select the correct `moxcms::Layout` when building transforms
/// and to validate that source colors are compatible with source profiles.
///
/// # Examples
///
/// ```
/// use appthere_color::ColorSpace;
///
/// let space = ColorSpace::Rgb;
/// assert_eq!(space.channel_count(), 3);
/// ```
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub enum ColorSpace {
    /// RGB color space (3 channels: red, green, blue).
    Rgb,
    /// CMYK color space (4 channels: cyan, magenta, yellow, key/black).
    Cmyk,
    /// CIE L*a*b* color space (3 channels: L, a, b).
    Lab,
    /// CIE XYZ color space (3 channels: X, Y, Z).
    Xyz,
    /// Grayscale color space (1 channel).
    Gray,
}

impl ColorSpace {
    /// Returns the number of channels in this color space.
    ///
    /// # Examples
    ///
    /// ```
    /// use appthere_color::ColorSpace;
    ///
    /// assert_eq!(ColorSpace::Rgb.channel_count(), 3);
    /// assert_eq!(ColorSpace::Cmyk.channel_count(), 4);
    /// assert_eq!(ColorSpace::Gray.channel_count(), 1);
    /// ```
    pub const fn channel_count(self) -> usize {
        match self {
            ColorSpace::Rgb => 3,
            ColorSpace::Cmyk => 4,
            ColorSpace::Lab => 3,
            ColorSpace::Xyz => 3,
            ColorSpace::Gray => 1,
        }
    }

    /// Returns the `moxcms::Layout` for this color space.
    ///
    /// CMYK uses `Layout::Rgba` because `moxcms` treats four-channel data
    /// with the same memory layout as RGBA.
    ///
    /// # Examples
    ///
    /// ```
    /// use appthere_color::ColorSpace;
    ///
    /// let layout = ColorSpace::Rgb.to_moxcms_layout();
    /// ```
    pub fn to_moxcms_layout(self) -> moxcms::Layout {
        match self {
            ColorSpace::Rgb => moxcms::Layout::Rgb,
            ColorSpace::Cmyk => moxcms::Layout::Rgba,
            ColorSpace::Lab => moxcms::Layout::Rgb,
            ColorSpace::Xyz => moxcms::Layout::Rgb,
            ColorSpace::Gray => moxcms::Layout::Gray,
        }
    }

    /// Attempts to create a [`ColorSpace`] from a `moxcms::DataColorSpace`.
    ///
    /// # Errors
    ///
    /// Returns [`ColorError::UnsupportedColorSpace`] if the data color space
    /// is not one of the five supported models (RGB, CMYK, Lab, XYZ, Gray).
    ///
    /// # Examples
    ///
    /// ```
    /// use appthere_color::ColorSpace;
    ///
    /// let space = ColorSpace::from_moxcms(moxcms::DataColorSpace::Rgb);
    /// assert!(space.is_ok());
    /// assert_eq!(space.unwrap(), ColorSpace::Rgb);
    /// ```
    pub fn from_moxcms(dcs: moxcms::DataColorSpace) -> ColorResult<Self> {
        match dcs {
            moxcms::DataColorSpace::Rgb => Ok(ColorSpace::Rgb),
            moxcms::DataColorSpace::Cmyk => Ok(ColorSpace::Cmyk),
            moxcms::DataColorSpace::Lab => Ok(ColorSpace::Lab),
            moxcms::DataColorSpace::Xyz => Ok(ColorSpace::Xyz),
            moxcms::DataColorSpace::Gray => Ok(ColorSpace::Gray),
            other => Err(ColorError::UnsupportedColorSpace(
                alloc::format!("{:?}", other),
            )),
        }
    }
}

impl core::fmt::Display for ColorSpace {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self {
            ColorSpace::Rgb => write!(f, "RGB"),
            ColorSpace::Cmyk => write!(f, "CMYK"),
            ColorSpace::Lab => write!(f, "Lab"),
            ColorSpace::Xyz => write!(f, "XYZ"),
            ColorSpace::Gray => write!(f, "Gray"),
        }
    }
}

extern crate alloc;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn channel_counts() {
        assert_eq!(ColorSpace::Rgb.channel_count(), 3);
        assert_eq!(ColorSpace::Cmyk.channel_count(), 4);
        assert_eq!(ColorSpace::Lab.channel_count(), 3);
        assert_eq!(ColorSpace::Xyz.channel_count(), 3);
        assert_eq!(ColorSpace::Gray.channel_count(), 1);
    }

    #[test]
    fn display_formatting() {
        assert_eq!(ColorSpace::Rgb.to_string(), "RGB");
        assert_eq!(ColorSpace::Cmyk.to_string(), "CMYK");
    }

    #[test]
    fn from_moxcms_supported() {
        assert_eq!(
            ColorSpace::from_moxcms(moxcms::DataColorSpace::Rgb).unwrap(),
            ColorSpace::Rgb
        );
        assert_eq!(
            ColorSpace::from_moxcms(moxcms::DataColorSpace::Cmyk).unwrap(),
            ColorSpace::Cmyk
        );
    }

    #[test]
    fn from_moxcms_unsupported() {
        // Hls is not one of our supported five
        let result = ColorSpace::from_moxcms(moxcms::DataColorSpace::Hls);
        assert!(matches!(result, Err(ColorError::UnsupportedColorSpace(_))));
    }
}
