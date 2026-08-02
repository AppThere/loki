//! Error types for the `appthere-color` crate.
//!
//! All fallible operations in this crate return [`ColorResult<T>`], which uses
//! [`ColorError`] as the error type. Error variants are specific enough to be
//! actionable by callers.

extern crate alloc;
use alloc::fmt;
use alloc::string::String;

/// The error type for color management operations.
///
/// Every fallible function in `appthere-color` returns this error type
/// through the [`ColorResult`] alias. Variants are designed to be actionable:
/// each one identifies a specific failure mode so callers can recover or
/// produce meaningful diagnostics.
#[derive(Debug, Clone, thiserror::Error)]
pub enum ColorError {
    /// Failed to parse an ICC profile from raw bytes.
    #[error("failed to parse ICC profile: {0}")]
    ProfileParse(String),

    /// A transform requires compatible profile classes but received
    /// incompatible ones.
    #[error("transform requires compatible profile classes, got {src} and {dst}")]
    IncompatibleProfiles {
        /// Description of the source profile class.
        src: String,
        /// Description of the destination profile class.
        dst: String,
    },

    /// A color component value is outside its valid range.
    #[error("color component {component} out of range: {value}")]
    ComponentOutOfRange {
        /// Name of the component (e.g. `"red"`, `"cyan"`, `"L"`).
        component: &'static str,
        /// The out-of-range value that was provided.
        value: f32,
    },

    /// A proofing configuration is missing required fields.
    #[error("proofing config is incomplete: {0}")]
    IncompleteProofingConfig(String),

    /// The color transform has not been built yet.
    #[error("transform not built: call .build() before transforming")]
    TransformNotBuilt,

    /// An error occurred during the ICC transform execution.
    #[error("transform execution failed: {0}")]
    TransformExecution(String),

    /// The requested color space is not supported for this operation.
    #[error("unsupported color space: {0}")]
    UnsupportedColorSpace(String),

    /// An I/O error occurred while reading a profile from a file path.
    #[cfg(feature = "std")]
    #[error("I/O error: {0}")]
    Io(String),

    /// A required profile is missing from the policy or registry.
    #[error("missing profile: {0}")]
    MissingProfile(String),
}

impl ColorError {
    /// Creates a [`ColorError::Io`] from a [`std::io::Error`].
    ///
    /// # Examples
    ///
    /// ```
    /// # #[cfg(feature = "std")]
    /// # {
    /// use appthere_color::ColorError;
    ///
    /// let io_err = std::io::Error::new(std::io::ErrorKind::NotFound, "file missing");
    /// let color_err = ColorError::from_io(io_err);
    /// assert!(matches!(color_err, ColorError::Io(_)));
    /// # }
    /// ```
    #[cfg(feature = "std")]
    pub fn from_io(err: std::io::Error) -> Self {
        ColorError::Io(fmt::format(format_args!("{}", err)))
    }
}

/// A type alias for `Result<T, ColorError>`.
///
/// This is the standard result type used throughout `appthere-color`.
///
/// # Examples
///
/// ```
/// use appthere_color::ColorResult;
///
/// fn parse_value(s: &str) -> ColorResult<f32> {
///     s.parse::<f32>().map_err(|e| {
///         appthere_color::ColorError::ComponentOutOfRange {
///             component: "value",
///             value: 0.0,
///         }
///     })
/// }
/// ```
pub type ColorResult<T> = Result<T, ColorError>;
