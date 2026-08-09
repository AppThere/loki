//! Builder for [`ProofingConfig`].
//!
//! Separated from `proofing.rs` to keep files under 300 lines.

extern crate alloc;

use moxcms::TransformOptions;

use crate::error::{ColorError, ColorResult};
use crate::profile::IccProfile;
use crate::proofing::{GamutWarning, ProofingConfig};
use crate::rendering_intent::RenderingIntent;

/// Builder for [`ProofingConfig`].
///
/// Created via [`ProofingConfig::builder()`].
pub struct ProofingConfigBuilder {
    pub(crate) source: Option<IccProfile>,
    pub(crate) simulation: Option<IccProfile>,
    pub(crate) display: Option<IccProfile>,
    pub(crate) simulation_intent: RenderingIntent,
    pub(crate) display_intent: RenderingIntent,
    pub(crate) gamut_warning: GamutWarning,
}

impl ProofingConfigBuilder {
    /// Sets the source profile.
    ///
    /// # Examples
    ///
    /// ```
    /// use appthere_color::{ProofingConfig, IccProfile};
    ///
    /// let builder = ProofingConfig::builder()
    ///     .source(&IccProfile::new_srgb());
    /// ```
    pub fn source(mut self, profile: &IccProfile) -> Self {
        self.source = Some(profile.clone());
        self
    }

    /// Sets the simulation (output device) profile.
    ///
    /// # Examples
    ///
    /// ```
    /// use appthere_color::{ProofingConfig, IccProfile};
    ///
    /// let builder = ProofingConfig::builder()
    ///     .simulation(&IccProfile::new_srgb());
    /// ```
    pub fn simulation(mut self, profile: &IccProfile) -> Self {
        self.simulation = Some(profile.clone());
        self
    }

    /// Sets the display profile.
    ///
    /// # Examples
    ///
    /// ```
    /// use appthere_color::{ProofingConfig, IccProfile};
    ///
    /// let builder = ProofingConfig::builder()
    ///     .display(&IccProfile::new_srgb());
    /// ```
    pub fn display(mut self, profile: &IccProfile) -> Self {
        self.display = Some(profile.clone());
        self
    }

    /// Sets the rendering intent for the source → simulation transform.
    ///
    /// # Examples
    ///
    /// ```
    /// use appthere_color::{ProofingConfig, RenderingIntent};
    ///
    /// let builder = ProofingConfig::builder()
    ///     .simulation_intent(RenderingIntent::Saturation);
    /// ```
    pub fn simulation_intent(mut self, intent: RenderingIntent) -> Self {
        self.simulation_intent = intent;
        self
    }

    /// Sets the rendering intent for the simulation → display transform.
    ///
    /// # Examples
    ///
    /// ```
    /// use appthere_color::{ProofingConfig, RenderingIntent};
    ///
    /// let builder = ProofingConfig::builder()
    ///     .display_intent(RenderingIntent::RelativeColorimetric);
    /// ```
    pub fn display_intent(mut self, intent: RenderingIntent) -> Self {
        self.display_intent = intent;
        self
    }

    /// Sets the gamut warning mode.
    ///
    /// # Examples
    ///
    /// ```
    /// use appthere_color::{ProofingConfig, GamutWarning};
    ///
    /// let builder = ProofingConfig::builder()
    ///     .gamut_warning(GamutWarning::None);
    /// ```
    pub fn gamut_warning(mut self, warning: GamutWarning) -> Self {
        self.gamut_warning = warning;
        self
    }

    /// Builds the proofing configuration, validating all required fields.
    ///
    /// # Errors
    ///
    /// - [`ColorError::IncompleteProofingConfig`] if any required profile
    ///   is missing.
    /// - [`ColorError::UnsupportedColorSpace`] if a profile's color space
    ///   is not supported.
    /// - [`ColorError::TransformExecution`] if `moxcms` cannot create
    ///   either transform.
    ///
    /// # Examples
    ///
    /// ```
    /// use appthere_color::{ProofingConfig, IccProfile, RenderingIntent};
    ///
    /// let result = ProofingConfig::builder()
    ///     .source(&IccProfile::new_srgb())
    ///     .simulation(&IccProfile::new_srgb())
    ///     .display(&IccProfile::new_srgb())
    ///     .build();
    /// assert!(result.is_ok());
    /// ```
    pub fn build(self) -> ColorResult<ProofingConfig> {
        let src = self.source.ok_or_else(|| {
            ColorError::IncompleteProofingConfig("source profile".into())
        })?;
        let sim = self.simulation.ok_or_else(|| {
            ColorError::IncompleteProofingConfig("simulation profile".into())
        })?;
        let disp = self.display.ok_or_else(|| {
            ColorError::IncompleteProofingConfig("display profile".into())
        })?;

        let src_space = src.color_space()?;
        let sim_space = sim.color_space()?;
        let disp_space = disp.color_space()?;

        let sim_opts = TransformOptions {
            rendering_intent: self.simulation_intent.to_moxcms(),
            ..TransformOptions::default()
        };
        let disp_opts = TransformOptions {
            rendering_intent: self.display_intent.to_moxcms(),
            ..TransformOptions::default()
        };

        let sim_exec = src
            .inner
            .create_transform_f32(
                src_space.to_moxcms_layout(),
                &sim.inner,
                sim_space.to_moxcms_layout(),
                sim_opts,
            )
            .map_err(|e| {
                ColorError::TransformExecution(alloc::format!("{:?}", e))
            })?;

        let disp_exec = sim
            .inner
            .create_transform_f32(
                sim_space.to_moxcms_layout(),
                &disp.inner,
                disp_space.to_moxcms_layout(),
                disp_opts,
            )
            .map_err(|e| {
                ColorError::TransformExecution(alloc::format!("{:?}", e))
            })?;

        ProofingConfig::new(
            sim_exec,
            disp_exec,
            self.gamut_warning,
            sim_space.channel_count(),
            src_space.channel_count(),
            disp_space.channel_count(),
        )
    }
}
