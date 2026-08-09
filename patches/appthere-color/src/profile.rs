//! ICC profile loading and inspection via `moxcms`.
//!
//! Wraps [`moxcms::ColorProfile`] to provide a safe, idiomatic interface for
//! loading ICC profiles from bytes or file paths, and inspecting their
//! color space and profile class.

extern crate alloc;
use alloc::string::String;

use crate::color_space::ColorSpace;
use crate::error::{ColorError, ColorResult};

/// An ICC color profile backed by `moxcms`.
///
/// `IccProfile` wraps a [`moxcms::ColorProfile`] and provides methods for
/// loading profiles from raw bytes or file paths, and inspecting profile
/// metadata such as color space and description.
///
/// # Examples
///
/// ```
/// use appthere_color::IccProfile;
///
/// let profile = IccProfile::new_srgb();
/// assert_eq!(profile.color_space().unwrap(), appthere_color::ColorSpace::Rgb);
/// ```
#[derive(Debug, Clone)]
pub struct IccProfile {
    pub(crate) inner: moxcms::ColorProfile,
}

impl IccProfile {
    /// Creates an IccProfile from raw ICC profile bytes.
    ///
    /// # Errors
    ///
    /// Returns [`ColorError::ProfileParse`] if the bytes do not contain a
    /// valid ICC profile.
    ///
    /// # Examples
    ///
    /// ```
    /// use appthere_color::IccProfile;
    ///
    /// let result = IccProfile::from_bytes(&[]);
    /// assert!(result.is_err());
    /// ```
    pub fn from_bytes(bytes: &[u8]) -> ColorResult<Self> {
        let inner = moxcms::ColorProfile::new_from_slice(bytes)
            .map_err(|e| ColorError::ProfileParse(alloc::format!("{:?}", e)))?;
        Ok(Self { inner })
    }

    /// Loads an ICC profile from a file path.
    ///
    /// # Errors
    ///
    /// Returns [`ColorError::Io`] if the file cannot be read, or
    /// [`ColorError::ProfileParse`] if the file contents are not a valid
    /// ICC profile.
    ///
    /// # Examples
    ///
    /// ```no_run
    /// use appthere_color::IccProfile;
    ///
    /// let profile = IccProfile::from_path("profiles/sRGB.icc").unwrap();
    /// ```
    #[cfg(feature = "std")]
    pub fn from_path(path: impl AsRef<std::path::Path>) -> ColorResult<Self> {
        let bytes = std::fs::read(path.as_ref()).map_err(ColorError::from_io)?;
        Self::from_bytes(&bytes)
    }

    /// Creates a built-in sRGB profile.
    ///
    /// # Examples
    ///
    /// ```
    /// use appthere_color::IccProfile;
    ///
    /// let profile = IccProfile::new_srgb();
    /// ```
    pub fn new_srgb() -> Self {
        Self {
            inner: moxcms::ColorProfile::new_srgb(),
        }
    }

    /// Creates a built-in Adobe RGB profile.
    ///
    /// # Examples
    ///
    /// ```
    /// use appthere_color::IccProfile;
    ///
    /// let profile = IccProfile::new_adobe_rgb();
    /// ```
    pub fn new_adobe_rgb() -> Self {
        Self {
            inner: moxcms::ColorProfile::new_adobe_rgb(),
        }
    }

    /// Creates a built-in Display P3 profile.
    ///
    /// # Examples
    ///
    /// ```
    /// use appthere_color::IccProfile;
    ///
    /// let profile = IccProfile::new_display_p3();
    /// ```
    pub fn new_display_p3() -> Self {
        Self {
            inner: moxcms::ColorProfile::new_display_p3(),
        }
    }

    /// Creates a built-in CIE Lab profile.
    ///
    /// # Examples
    ///
    /// ```
    /// use appthere_color::IccProfile;
    ///
    /// let profile = IccProfile::new_lab();
    /// ```
    pub fn new_lab() -> Self {
        Self {
            inner: moxcms::ColorProfile::new_lab(),
        }
    }

    /// Creates a built-in grayscale profile with the specified gamma.
    ///
    /// # Examples
    ///
    /// ```
    /// use appthere_color::IccProfile;
    ///
    /// let profile = IccProfile::new_gray(2.2);
    /// ```
    pub fn new_gray(gamma: f32) -> Self {
        Self {
            inner: moxcms::ColorProfile::new_gray_with_gamma(gamma),
        }
    }

    /// Returns the [`ColorSpace`] of this profile.
    ///
    /// # Errors
    ///
    /// Returns [`ColorError::UnsupportedColorSpace`] if the profile's
    /// data color space is not one of the five supported models.
    ///
    /// # Examples
    ///
    /// ```
    /// use appthere_color::{IccProfile, ColorSpace};
    ///
    /// let srgb = IccProfile::new_srgb();
    /// assert_eq!(srgb.color_space().unwrap(), ColorSpace::Rgb);
    /// ```
    pub fn color_space(&self) -> ColorResult<ColorSpace> {
        ColorSpace::from_moxcms(self.inner.color_space)
    }

    /// Returns the profile description as a string, if available.
    ///
    /// # Examples
    ///
    /// ```
    /// use appthere_color::IccProfile;
    ///
    /// let profile = IccProfile::new_srgb();
    /// // Built-in profiles may or may not have a description.
    /// let _desc = profile.description();
    /// ```
    pub fn description(&self) -> Option<String> {
        self.inner
            .description
            .as_ref()
            .map(|t| alloc::format!("{:?}", t))
    }

    /// Returns a reference to the underlying `moxcms::ColorProfile`.
    ///
    /// # Examples
    ///
    /// ```
    /// use appthere_color::IccProfile;
    ///
    /// let profile = IccProfile::new_srgb();
    /// let inner = profile.as_moxcms();
    /// ```
    pub fn as_moxcms(&self) -> &moxcms::ColorProfile {
        &self.inner
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn srgb_profile_is_rgb() {
        let p = IccProfile::new_srgb();
        assert_eq!(p.color_space().unwrap(), ColorSpace::Rgb);
    }

    #[test]
    fn lab_profile_is_lab() {
        let p = IccProfile::new_lab();
        assert_eq!(p.color_space().unwrap(), ColorSpace::Lab);
    }

    #[test]
    fn gray_profile_is_gray() {
        let p = IccProfile::new_gray(2.2);
        assert_eq!(p.color_space().unwrap(), ColorSpace::Gray);
    }

    #[test]
    fn from_empty_bytes_returns_error() {
        let result = IccProfile::from_bytes(&[]);
        assert!(matches!(result, Err(ColorError::ProfileParse(_))));
    }

    #[test]
    fn from_garbage_bytes_returns_error() {
        let result = IccProfile::from_bytes(&[0xFF, 0x00, 0xAB, 0xCD]);
        assert!(matches!(result, Err(ColorError::ProfileParse(_))));
    }
}
