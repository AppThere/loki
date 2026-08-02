//! Document-level color management policy.
//!
//! Provides [`ColorPolicy`] and [`OutputIntent`] which represent format-agnostic
//! document-level color management configuration. This crate provides the data
//! model only — format-specific serialization belongs in the consuming crate.

extern crate alloc;
use alloc::collections::BTreeMap;
use alloc::string::String;

use crate::profile::IccProfile;
use crate::rendering_intent::RenderingIntent;

/// Describes the intended output condition for a document.
///
/// Maps conceptually to PDF/X `/OutputIntents`, ODF `draw:color-profile`, or
/// embedded ICC profiles in EPUB image resources.
///
/// # Examples
///
/// ```
/// use appthere_color::{OutputIntent, IccProfile};
///
/// let intent = OutputIntent::new(
///     "FOGRA39".to_string(),
///     IccProfile::new_srgb(),
/// );
/// assert_eq!(intent.name(), "FOGRA39");
/// ```
#[derive(Debug, Clone)]
pub struct OutputIntent {
    name: String,
    profile: IccProfile,
    condition: Option<String>,
}

impl OutputIntent {
    /// Creates a new output intent with the given name and profile.
    ///
    /// # Examples
    ///
    /// ```
    /// use appthere_color::{OutputIntent, IccProfile};
    ///
    /// let oi = OutputIntent::new("ISOcoated_v2".to_string(), IccProfile::new_srgb());
    /// ```
    pub fn new(name: String, profile: IccProfile) -> Self {
        Self {
            name,
            profile,
            condition: None,
        }
    }

    /// Sets the output condition identifier (e.g. a registered condition name).
    ///
    /// # Examples
    ///
    /// ```
    /// use appthere_color::{OutputIntent, IccProfile};
    ///
    /// let oi = OutputIntent::new("FOGRA39".to_string(), IccProfile::new_srgb())
    ///     .with_condition("FOGRA39L".to_string());
    /// assert_eq!(oi.condition(), Some("FOGRA39L"));
    /// ```
    pub fn with_condition(mut self, condition: String) -> Self {
        self.condition = Some(condition);
        self
    }

    /// Returns the output intent name.
    ///
    /// # Examples
    ///
    /// ```
    /// use appthere_color::{OutputIntent, IccProfile};
    ///
    /// let oi = OutputIntent::new("test".to_string(), IccProfile::new_srgb());
    /// assert_eq!(oi.name(), "test");
    /// ```
    pub fn name(&self) -> &str {
        &self.name
    }

    /// Returns a reference to the output intent's ICC profile.
    ///
    /// # Examples
    ///
    /// ```
    /// use appthere_color::{OutputIntent, IccProfile};
    ///
    /// let oi = OutputIntent::new("sRGB".to_string(), IccProfile::new_srgb());
    /// let _profile = oi.profile();
    /// ```
    pub fn profile(&self) -> &IccProfile {
        &self.profile
    }

    /// Returns the output condition string, if set.
    ///
    /// # Examples
    ///
    /// ```
    /// use appthere_color::{OutputIntent, IccProfile};
    ///
    /// let oi = OutputIntent::new("test".to_string(), IccProfile::new_srgb());
    /// assert_eq!(oi.condition(), None);
    /// ```
    pub fn condition(&self) -> Option<&str> {
        self.condition.as_deref()
    }
}

/// Document-level color management policy.
///
/// Captures the output intent, fallback profile, default rendering intent,
/// and a registry of named embedded profiles. This is a format-agnostic
/// representation — serialization to PDF/X, ODF, or EPUB is handled by the
/// consuming crate.
///
/// # Examples
///
/// ```
/// use appthere_color::{ColorPolicy, OutputIntent, IccProfile, RenderingIntent};
///
/// let policy = ColorPolicy::builder()
///     .output_intent(OutputIntent::new(
///         "sRGB".to_string(),
///         IccProfile::new_srgb(),
///     ))
///     .default_intent(RenderingIntent::RelativeColorimetric)
///     .build();
/// ```
#[derive(Debug, Clone)]
pub struct ColorPolicy {
    output_intent: Option<OutputIntent>,
    fallback_profile: Option<IccProfile>,
    default_intent: RenderingIntent,
    embedded_profiles: BTreeMap<String, IccProfile>,
}

impl ColorPolicy {
    /// Creates a new [`ColorPolicyBuilder`].
    ///
    /// # Examples
    ///
    /// ```
    /// use appthere_color::ColorPolicy;
    ///
    /// let builder = ColorPolicy::builder();
    /// ```
    pub fn builder() -> ColorPolicyBuilder {
        ColorPolicyBuilder {
            output_intent: None,
            fallback_profile: None,
            default_intent: RenderingIntent::Perceptual,
            embedded_profiles: BTreeMap::new(),
        }
    }

    /// Returns the output intent, if set.
    ///
    /// # Examples
    ///
    /// ```
    /// use appthere_color::ColorPolicy;
    ///
    /// let policy = ColorPolicy::builder().build();
    /// assert!(policy.output_intent().is_none());
    /// ```
    pub fn output_intent(&self) -> Option<&OutputIntent> {
        self.output_intent.as_ref()
    }

    /// Returns the fallback profile, if set.
    ///
    /// # Examples
    ///
    /// ```
    /// use appthere_color::ColorPolicy;
    ///
    /// let policy = ColorPolicy::builder().build();
    /// assert!(policy.fallback_profile().is_none());
    /// ```
    pub fn fallback_profile(&self) -> Option<&IccProfile> {
        self.fallback_profile.as_ref()
    }

    /// Returns the default rendering intent for this policy.
    ///
    /// # Examples
    ///
    /// ```
    /// use appthere_color::{ColorPolicy, RenderingIntent};
    ///
    /// let policy = ColorPolicy::builder().build();
    /// assert_eq!(policy.default_intent(), RenderingIntent::Perceptual);
    /// ```
    pub fn default_intent(&self) -> RenderingIntent {
        self.default_intent
    }

    /// Looks up a named embedded profile in the registry.
    ///
    /// # Examples
    ///
    /// ```
    /// use appthere_color::{ColorPolicy, IccProfile};
    ///
    /// let policy = ColorPolicy::builder()
    ///     .embed_profile("sRGB".to_string(), IccProfile::new_srgb())
    ///     .build();
    /// assert!(policy.get_profile("sRGB").is_some());
    /// assert!(policy.get_profile("AdobeRGB").is_none());
    /// ```
    pub fn get_profile(&self, name: &str) -> Option<&IccProfile> {
        self.embedded_profiles.get(name)
    }
}

/// Builder for [`ColorPolicy`].
///
/// Created via [`ColorPolicy::builder()`].
#[derive(Debug)]
pub struct ColorPolicyBuilder {
    output_intent: Option<OutputIntent>,
    fallback_profile: Option<IccProfile>,
    default_intent: RenderingIntent,
    embedded_profiles: BTreeMap<String, IccProfile>,
}

impl ColorPolicyBuilder {
    /// Sets the output intent.
    ///
    /// # Examples
    ///
    /// ```
    /// use appthere_color::{ColorPolicy, OutputIntent, IccProfile};
    ///
    /// let builder = ColorPolicy::builder()
    ///     .output_intent(OutputIntent::new("sRGB".into(), IccProfile::new_srgb()));
    /// ```
    pub fn output_intent(mut self, intent: OutputIntent) -> Self {
        self.output_intent = Some(intent);
        self
    }

    /// Sets the fallback profile.
    ///
    /// # Examples
    ///
    /// ```
    /// use appthere_color::{ColorPolicy, IccProfile};
    ///
    /// let builder = ColorPolicy::builder()
    ///     .fallback_profile(IccProfile::new_srgb());
    /// ```
    pub fn fallback_profile(mut self, profile: IccProfile) -> Self {
        self.fallback_profile = Some(profile);
        self
    }

    /// Sets the default rendering intent.
    ///
    /// # Examples
    ///
    /// ```
    /// use appthere_color::{ColorPolicy, RenderingIntent};
    ///
    /// let builder = ColorPolicy::builder()
    ///     .default_intent(RenderingIntent::Saturation);
    /// ```
    pub fn default_intent(mut self, intent: RenderingIntent) -> Self {
        self.default_intent = intent;
        self
    }

    /// Registers a named embedded profile.
    ///
    /// # Examples
    ///
    /// ```
    /// use appthere_color::{ColorPolicy, IccProfile};
    ///
    /// let builder = ColorPolicy::builder()
    ///     .embed_profile("logo".to_string(), IccProfile::new_srgb());
    /// ```
    pub fn embed_profile(mut self, name: String, profile: IccProfile) -> Self {
        self.embedded_profiles.insert(name, profile);
        self
    }

    /// Builds the [`ColorPolicy`].
    ///
    /// # Examples
    ///
    /// ```
    /// use appthere_color::ColorPolicy;
    ///
    /// let policy = ColorPolicy::builder().build();
    /// ```
    pub fn build(self) -> ColorPolicy {
        ColorPolicy {
            output_intent: self.output_intent,
            fallback_profile: self.fallback_profile,
            default_intent: self.default_intent,
            embedded_profiles: self.embedded_profiles,
        }
    }
}
