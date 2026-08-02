//! Soft proofing configuration and chained transform execution.
//!
//! Simulates how a document will appear when printed on a specific output
//! device by chaining two transforms through an intermediate buffer.

extern crate alloc;

use alloc::boxed::Box;
use alloc::vec;

use crate::error::{ColorError, ColorResult};
use crate::proofing_builder::ProofingConfigBuilder;
use crate::rendering_intent::RenderingIntent;

/// Gamut warning indicator for out-of-gamut pixels.
///
/// When soft proofing, pixels that fall outside the output device's gamut
/// can be flagged with a warning color.
///
/// # Examples
///
/// ```
/// use appthere_color::{GamutWarning, RgbColor};
///
/// let warning = GamutWarning::Replace(RgbColor::new(1.0, 0.0, 1.0));
/// ```
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum GamutWarning {
    /// No gamut warning — out-of-gamut colors pass through unchanged.
    None,
    /// Replace out-of-gamut pixels with the specified RGB warning color.
    Replace(crate::color_value::RgbColor),
}

impl Default for GamutWarning {
    /// Returns [`GamutWarning::None`] as the default.
    fn default() -> Self {
        GamutWarning::None
    }
}

/// Configuration for soft proofing transforms.
///
/// Built via [`ProofingConfig::builder()`], this struct captures the profiles,
/// intents, and optional gamut warning needed for a two-stage proofing pipeline.
///
/// # Examples
///
/// ```
/// use appthere_color::{ProofingConfig, IccProfile, RenderingIntent};
///
/// let config = ProofingConfig::builder()
///     .source(&IccProfile::new_srgb())
///     .simulation(&IccProfile::new_srgb())
///     .display(&IccProfile::new_srgb())
///     .simulation_intent(RenderingIntent::Perceptual)
///     .build()
///     .unwrap();
/// ```
pub struct ProofingConfig {
    sim_executor: Box<moxcms::TransformF32BitExecutor>,
    disp_executor: Box<moxcms::TransformF32BitExecutor>,
    gamut_warning: GamutWarning,
    sim_channels: usize,
    src_channels: usize,
    dst_channels: usize,
}

impl ProofingConfig {
    /// Creates a new [`ProofingConfigBuilder`].
    ///
    /// # Examples
    ///
    /// ```
    /// use appthere_color::ProofingConfig;
    ///
    /// let builder = ProofingConfig::builder();
    /// ```
    pub fn builder() -> ProofingConfigBuilder {
        ProofingConfigBuilder {
            source: None,
            simulation: None,
            display: None,
            simulation_intent: RenderingIntent::Perceptual,
            display_intent: RenderingIntent::RelativeColorimetric,
            gamut_warning: GamutWarning::None,
        }
    }

    /// Internal constructor used by the builder.
    pub(crate) fn new(
        sim_executor: Box<moxcms::TransformF32BitExecutor>,
        disp_executor: Box<moxcms::TransformF32BitExecutor>,
        gamut_warning: GamutWarning,
        sim_channels: usize,
        src_channels: usize,
        dst_channels: usize,
    ) -> ColorResult<Self> {
        Ok(Self {
            sim_executor,
            disp_executor,
            gamut_warning,
            sim_channels,
            src_channels,
            dst_channels,
        })
    }

    /// Executes the two-stage proofing transform on pixel data.
    ///
    /// Transform 1: source → simulation profile (simulation_intent).
    /// Transform 2: simulation → display profile (display_intent).
    ///
    /// # Errors
    ///
    /// Returns [`ColorError::TransformExecution`] if either stage fails.
    ///
    /// # Examples
    ///
    /// ```
    /// use appthere_color::{ProofingConfig, IccProfile, RenderingIntent};
    ///
    /// let config = ProofingConfig::builder()
    ///     .source(&IccProfile::new_srgb())
    ///     .simulation(&IccProfile::new_srgb())
    ///     .display(&IccProfile::new_srgb())
    ///     .build()
    ///     .unwrap();
    ///
    /// let input = [0.5_f32, 0.3, 0.8];
    /// let mut output = [0.0_f32; 3];
    /// config.proof(&input, &mut output).unwrap();
    /// ```
    pub fn proof(&self, src: &[f32], dst: &mut [f32]) -> ColorResult<()> {
        let mut intermediate = vec![0.0_f32; self.sim_channels];

        self.sim_executor
            .transform(src, &mut intermediate)
            .map_err(|e| {
                ColorError::TransformExecution(alloc::format!("{:?}", e))
            })?;

        self.disp_executor
            .transform(&intermediate, dst)
            .map_err(|e| {
                ColorError::TransformExecution(alloc::format!("{:?}", e))
            })?;

        Ok(())
    }

    /// Returns the gamut warning mode configured for this proofing config.
    ///
    /// # Examples
    ///
    /// ```
    /// use appthere_color::{ProofingConfig, IccProfile, GamutWarning};
    ///
    /// let config = ProofingConfig::builder()
    ///     .source(&IccProfile::new_srgb())
    ///     .simulation(&IccProfile::new_srgb())
    ///     .display(&IccProfile::new_srgb())
    ///     .build()
    ///     .unwrap();
    /// assert_eq!(config.gamut_warning(), GamutWarning::None);
    /// ```
    pub fn gamut_warning(&self) -> GamutWarning {
        self.gamut_warning
    }

    /// Returns the number of channels in the source color space.
    ///
    /// # Examples
    ///
    /// ```
    /// use appthere_color::{ProofingConfig, IccProfile};
    ///
    /// let config = ProofingConfig::builder()
    ///     .source(&IccProfile::new_srgb())
    ///     .simulation(&IccProfile::new_srgb())
    ///     .display(&IccProfile::new_srgb())
    ///     .build()
    ///     .unwrap();
    /// assert_eq!(config.source_channels(), 3);
    /// ```
    pub fn source_channels(&self) -> usize {
        self.src_channels
    }

    /// Returns the number of channels in the display color space.
    ///
    /// # Examples
    ///
    /// ```
    /// use appthere_color::{ProofingConfig, IccProfile};
    ///
    /// let config = ProofingConfig::builder()
    ///     .source(&IccProfile::new_srgb())
    ///     .simulation(&IccProfile::new_srgb())
    ///     .display(&IccProfile::new_srgb())
    ///     .build()
    ///     .unwrap();
    /// assert_eq!(config.destination_channels(), 3);
    /// ```
    pub fn destination_channels(&self) -> usize {
        self.dst_channels
    }
}
