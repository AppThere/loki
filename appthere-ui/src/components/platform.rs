// SPDX-License-Identifier: Apache-2.0

//! Platform identifier for shell chrome adaptation.

/// Identifies the host platform so shell components can adapt their layout.
///
/// Passed as a prop to platform-sensitive components such as
/// [`crate::components::AtTitleBar`]. Resolve it in the application's root
/// component with [`Platform::detect`] and pass the variant down via props.
#[derive(Clone, Copy, PartialEq, Eq, Default, Debug)]
pub enum Platform {
    /// Windows desktop (default).
    #[default]
    Windows,
    /// macOS desktop — title bar leaves room for traffic light buttons.
    MacOs,
    /// Linux desktop.
    Linux,
    /// Android mobile.
    Android,
    /// iOS mobile.
    Ios,
}

impl Platform {
    /// The compile-target host platform (`std::env::consts::OS`). Unknown
    /// targets fall back to the Windows chrome metrics (the default).
    #[must_use]
    pub fn detect() -> Self {
        Self::from_os_name(std::env::consts::OS)
    }

    /// Maps an `std::env::consts::OS`-style name to a variant (pure — the
    /// testable core of [`Self::detect`]).
    #[must_use]
    pub fn from_os_name(os: &str) -> Self {
        match os {
            "macos" => Self::MacOs,
            "linux" => Self::Linux,
            "android" => Self::Android,
            "ios" => Self::Ios,
            // "windows" and anything unrecognised use the default chrome.
            _ => Self::Windows,
        }
    }

    /// Returns `true` for desktop platforms (Windows, macOS, Linux).
    pub fn is_desktop(self) -> bool {
        matches!(self, Self::Windows | Self::MacOs | Self::Linux)
    }

    /// Returns `true` for mobile platforms (Android, iOS).
    pub fn is_mobile(self) -> bool {
        matches!(self, Self::Android | Self::Ios)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn os_names_map_to_variants() {
        assert_eq!(Platform::from_os_name("macos"), Platform::MacOs);
        assert_eq!(Platform::from_os_name("linux"), Platform::Linux);
        assert_eq!(Platform::from_os_name("android"), Platform::Android);
        assert_eq!(Platform::from_os_name("ios"), Platform::Ios);
        assert_eq!(Platform::from_os_name("windows"), Platform::Windows);
        assert_eq!(Platform::from_os_name("freebsd"), Platform::Windows);
    }

    #[test]
    fn detect_matches_the_compile_target() {
        assert_eq!(
            Platform::detect(),
            Platform::from_os_name(std::env::consts::OS)
        );
    }
}
