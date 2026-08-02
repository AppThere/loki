//! Color transforms between ICC profiles.
//!
//! Provides [`ColorTransform`] which uses a builder pattern to configure and
//! execute profile-to-profile color conversions via `moxcms`.

extern crate alloc;

use alloc::boxed::Box;

use moxcms::TransformOptions;
use crate::error::{ColorError, ColorResult};
use crate::profile::IccProfile;
use crate::rendering_intent::RenderingIntent;

/// A builder and executor for ICC profile-to-profile color transforms.
///
/// Use the builder pattern to configure source and destination profiles,
/// rendering intent, and then call [`build`](ColorTransformBuilder::build) to create
/// the transform. After building, call [`transform`](ColorTransform::transform)
/// to convert color data.
///
/// # Examples
///
/// ```
/// use appthere_color::{ColorTransform, IccProfile, RenderingIntent};
///
/// let src = IccProfile::new_srgb();
/// let dst = IccProfile::new_adobe_rgb();
///
/// let xform = ColorTransform::builder()
///     .source(&src)
///     .destination(&dst)
///     .intent(RenderingIntent::Perceptual)
///     .build()
///     .unwrap();
///
/// let input = [0.5_f32, 0.2, 0.8];
/// let mut output = [0.0_f32; 3];
/// xform.transform(&input, &mut output).unwrap();
/// ```
pub struct ColorTransform {
    executor: Box<moxcms::TransformF32BitExecutor>,
    src_channels: usize,
    dst_channels: usize,
}

impl ColorTransform {
    /// Creates a new [`ColorTransformBuilder`].
    ///
    /// # Examples
    ///
    /// ```
    /// use appthere_color::ColorTransform;
    ///
    /// let builder = ColorTransform::builder();
    /// ```
    pub fn builder() -> ColorTransformBuilder {
        ColorTransformBuilder {
            source: None,
            destination: None,
            intent: RenderingIntent::Perceptual,
        }
    }

    /// Transforms color data from source to destination color space.
    ///
    /// The `src` slice must contain a whole number of pixels in the source
    /// color space layout, and `dst` must be large enough to hold the same
    /// number of pixels in the destination layout.
    ///
    /// # Errors
    ///
    /// Returns [`ColorError::TransformExecution`] if the underlying `moxcms`
    /// transform fails (e.g., mismatched buffer sizes).
    ///
    /// # Examples
    ///
    /// ```
    /// use appthere_color::{ColorTransform, IccProfile, RenderingIntent};
    ///
    /// let src = IccProfile::new_srgb();
    /// let dst = IccProfile::new_adobe_rgb();
    /// let xform = ColorTransform::builder()
    ///     .source(&src)
    ///     .destination(&dst)
    ///     .intent(RenderingIntent::RelativeColorimetric)
    ///     .build()
    ///     .unwrap();
    ///
    /// let input = [1.0_f32, 0.0, 0.0];
    /// let mut output = [0.0_f32; 3];
    /// xform.transform(&input, &mut output).unwrap();
    /// ```
    pub fn transform(&self, src: &[f32], dst: &mut [f32]) -> ColorResult<()> {
        self.executor
            .transform(src, dst)
            .map_err(|e| ColorError::TransformExecution(alloc::format!("{:?}", e)))
    }

    /// Returns the number of channels in the source color space.
    ///
    /// # Examples
    ///
    /// ```
    /// use appthere_color::{ColorTransform, IccProfile, RenderingIntent};
    ///
    /// let xform = ColorTransform::builder()
    ///     .source(&IccProfile::new_srgb())
    ///     .destination(&IccProfile::new_adobe_rgb())
    ///     .build()
    ///     .unwrap();
    /// assert_eq!(xform.source_channels(), 3);
    /// ```
    pub fn source_channels(&self) -> usize {
        self.src_channels
    }

    /// Returns the number of channels in the destination color space.
    ///
    /// # Examples
    ///
    /// ```
    /// use appthere_color::{ColorTransform, IccProfile, RenderingIntent};
    ///
    /// let xform = ColorTransform::builder()
    ///     .source(&IccProfile::new_srgb())
    ///     .destination(&IccProfile::new_adobe_rgb())
    ///     .build()
    ///     .unwrap();
    /// assert_eq!(xform.destination_channels(), 3);
    /// ```
    pub fn destination_channels(&self) -> usize {
        self.dst_channels
    }
}

/// Builder for [`ColorTransform`].
///
/// Created via [`ColorTransform::builder()`].
pub struct ColorTransformBuilder {
    source: Option<IccProfile>,
    destination: Option<IccProfile>,
    intent: RenderingIntent,
}

impl ColorTransformBuilder {
    /// Sets the source ICC profile.
    ///
    /// # Examples
    ///
    /// ```
    /// use appthere_color::{ColorTransform, IccProfile};
    ///
    /// let builder = ColorTransform::builder()
    ///     .source(&IccProfile::new_srgb());
    /// ```
    pub fn source(mut self, profile: &IccProfile) -> Self {
        self.source = Some(profile.clone());
        self
    }

    /// Sets the destination ICC profile.
    ///
    /// # Examples
    ///
    /// ```
    /// use appthere_color::{ColorTransform, IccProfile};
    ///
    /// let builder = ColorTransform::builder()
    ///     .destination(&IccProfile::new_adobe_rgb());
    /// ```
    pub fn destination(mut self, profile: &IccProfile) -> Self {
        self.destination = Some(profile.clone());
        self
    }

    /// Sets the rendering intent for the transform.
    ///
    /// # Examples
    ///
    /// ```
    /// use appthere_color::{ColorTransform, RenderingIntent};
    ///
    /// let builder = ColorTransform::builder()
    ///     .intent(RenderingIntent::Saturation);
    /// ```
    pub fn intent(mut self, intent: RenderingIntent) -> Self {
        self.intent = intent;
        self
    }

    /// Builds the color transform, validating all required fields.
    ///
    /// # Errors
    ///
    /// - [`ColorError::TransformNotBuilt`] if source or destination profile is
    ///   missing.
    /// - [`ColorError::UnsupportedColorSpace`] if either profile's color space
    ///   is not supported.
    /// - [`ColorError::TransformExecution`] if `moxcms` cannot create the
    ///   transform.
    ///
    /// # Examples
    ///
    /// ```
    /// use appthere_color::{ColorTransform, IccProfile};
    ///
    /// let xform = ColorTransform::builder()
    ///     .source(&IccProfile::new_srgb())
    ///     .destination(&IccProfile::new_adobe_rgb())
    ///     .build();
    /// assert!(xform.is_ok());
    /// ```
    pub fn build(self) -> ColorResult<ColorTransform> {
        let src = self
            .source
            .ok_or(ColorError::TransformNotBuilt)?;
        let dst = self
            .destination
            .ok_or(ColorError::TransformNotBuilt)?;

        let src_space = src.color_space()?;
        let dst_space = dst.color_space()?;

        let src_layout = src_space.to_moxcms_layout();
        let dst_layout = dst_space.to_moxcms_layout();

        let options = TransformOptions {
            rendering_intent: self.intent.to_moxcms(),
            ..TransformOptions::default()
        };

        let executor = src
            .inner
            .create_transform_f32(src_layout, &dst.inner, dst_layout, options)
            .map_err(|e| {
                ColorError::TransformExecution(alloc::format!("{:?}", e))
            })?;

        Ok(ColorTransform {
            executor,
            src_channels: src_space.channel_count(),
            dst_channels: dst_space.channel_count(),
        })
    }
}


#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn build_without_source_fails() {
        let result = ColorTransform::builder()
            .destination(&IccProfile::new_srgb())
            .build();
        assert!(matches!(result, Err(ColorError::TransformNotBuilt)));
    }

    #[test]
    fn build_without_destination_fails() {
        let result = ColorTransform::builder()
            .source(&IccProfile::new_srgb())
            .build();
        assert!(matches!(result, Err(ColorError::TransformNotBuilt)));
    }

    #[test]
    fn srgb_to_adobe_rgb_transform() {
        let src = IccProfile::new_srgb();
        let dst = IccProfile::new_adobe_rgb();
        let xform = ColorTransform::builder()
            .source(&src)
            .destination(&dst)
            .build()
            .unwrap();

        let input = [1.0_f32, 0.0, 0.0];
        let mut output = [0.0_f32; 3];
        xform.transform(&input, &mut output).unwrap();

        // Red in sRGB should produce a valid color in Adobe RGB
        assert!(output[0] > 0.0);
    }
}
