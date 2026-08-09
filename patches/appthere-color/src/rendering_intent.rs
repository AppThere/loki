//! Rendering intent selection for ICC color transforms.
//!
//! Defines the four ICC rendering intents that control how out-of-gamut colors
//! are mapped during profile-to-profile conversion.

/// ICC rendering intent controlling out-of-gamut color mapping.
///
/// When converting colors between profiles with different gamuts, the rendering
/// intent determines how colors outside the destination gamut are handled.
/// Each intent makes a different trade-off between accuracy and aesthetics.
///
/// # Examples
///
/// ```
/// use appthere_color::RenderingIntent;
///
/// let intent = RenderingIntent::Perceptual;
/// assert_eq!(intent, RenderingIntent::Perceptual);
/// ```
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub enum RenderingIntent {
    /// Perceptual intent — optimizes for pleasing visual appearance.
    ///
    /// Compresses the entire source gamut to fit within the destination gamut,
    /// preserving the relationships between colors. Best for photographic images
    /// where visual quality matters more than exact color matching.
    Perceptual,

    /// Relative colorimetric intent — matches in-gamut colors exactly.
    ///
    /// Maps the source white point to the destination white point and maps
    /// in-gamut colors exactly. Out-of-gamut colors are clipped to the nearest
    /// reproducible color. Best for proofing and logo colors.
    RelativeColorimetric,

    /// Saturation intent — maximizes color vividness.
    ///
    /// Maps saturated source colors to saturated destination colors, prioritizing
    /// vividness over accuracy. Best for business graphics and charts where
    /// impact matters more than fidelity.
    Saturation,

    /// Absolute colorimetric intent — preserves exact colorimetry.
    ///
    /// Like relative colorimetric but does not adjust for the destination
    /// white point. Colors are reproduced with absolute fidelity, including
    /// the media white point simulation. Best for exact color matching and
    /// substrate simulation in proofing.
    AbsoluteColorimetric,
}

impl RenderingIntent {
    /// Converts this rendering intent to the corresponding `moxcms` intent.
    ///
    /// # Examples
    ///
    /// ```
    /// use appthere_color::RenderingIntent;
    ///
    /// let intent = RenderingIntent::Perceptual;
    /// let moxcms_intent = intent.to_moxcms();
    /// ```
    pub fn to_moxcms(self) -> moxcms::RenderingIntent {
        match self {
            RenderingIntent::Perceptual => moxcms::RenderingIntent::Perceptual,
            RenderingIntent::RelativeColorimetric => {
                moxcms::RenderingIntent::RelativeColorimetric
            }
            RenderingIntent::Saturation => moxcms::RenderingIntent::Saturation,
            RenderingIntent::AbsoluteColorimetric => {
                moxcms::RenderingIntent::AbsoluteColorimetric
            }
        }
    }

    /// Creates a rendering intent from the corresponding `moxcms` intent.
    ///
    /// # Examples
    ///
    /// ```
    /// use appthere_color::RenderingIntent;
    ///
    /// let intent = RenderingIntent::from_moxcms(moxcms::RenderingIntent::Perceptual);
    /// assert_eq!(intent, RenderingIntent::Perceptual);
    /// ```
    pub fn from_moxcms(intent: moxcms::RenderingIntent) -> Self {
        match intent {
            moxcms::RenderingIntent::Perceptual => RenderingIntent::Perceptual,
            moxcms::RenderingIntent::RelativeColorimetric => {
                RenderingIntent::RelativeColorimetric
            }
            moxcms::RenderingIntent::Saturation => RenderingIntent::Saturation,
            moxcms::RenderingIntent::AbsoluteColorimetric => {
                RenderingIntent::AbsoluteColorimetric
            }
        }
    }
}

impl Default for RenderingIntent {
    /// Returns [`RenderingIntent::Perceptual`] as the default.
    fn default() -> Self {
        RenderingIntent::Perceptual
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_is_perceptual() {
        assert_eq!(RenderingIntent::default(), RenderingIntent::Perceptual);
    }

    #[test]
    fn round_trip_through_moxcms() {
        let intents = [
            RenderingIntent::Perceptual,
            RenderingIntent::RelativeColorimetric,
            RenderingIntent::Saturation,
            RenderingIntent::AbsoluteColorimetric,
        ];
        for intent in intents {
            let mox = intent.to_moxcms();
            let back = RenderingIntent::from_moxcms(mox);
            assert_eq!(intent, back);
        }
    }
}
