// SPDX-License-Identifier: Apache-2.0

//! Platform identifier for shell chrome adaptation.

/// Identifies the host platform so shell components can adapt their layout.
///
/// Passed as a prop to platform-sensitive components such as [`crate::components::AtTitleBar`].
///
/// # Note
///
/// Actual platform detection belongs in the application crate.
/// // TODO(platform): wire to a real `std::env::consts::OS` or `cfg!(target_os)` check
/// in the application's root component and pass the resolved variant down via props.
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
    /// Returns `true` for desktop platforms (Windows, macOS, Linux).
    pub fn is_desktop(self) -> bool {
        matches!(self, Self::Windows | Self::MacOs | Self::Linux)
    }

    /// Returns `true` for mobile platforms (Android, iOS).
    pub fn is_mobile(self) -> bool {
        matches!(self, Self::Android | Self::Ios)
    }
}
