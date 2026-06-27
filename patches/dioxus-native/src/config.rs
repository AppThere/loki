use dioxus_core::LaunchConfig;
use winit::window::WindowAttributes;

/// The configuration for the desktop application.
pub struct Config {
    pub(crate) window_attributes: WindowAttributes,
    /// Extra font blobs (TTF/OTF/TTC bytes) registered into the renderer's font
    /// collection at startup, in addition to the system fonts. Use this to bundle
    /// application fonts so they resolve synchronously on every platform, instead
    /// of relying on the asynchronous `@font-face` `data:` URI fetch (which is
    /// unreliable on Android).
    pub(crate) font_blobs: Vec<Vec<u8>>,
}

impl LaunchConfig for Config {}

impl Default for Config {
    fn default() -> Self {
        Self {
            window_attributes: WindowAttributes::default().with_title(
                dioxus_cli_config::app_title().unwrap_or_else(|| "Dioxus App".to_string()),
            ),
            font_blobs: Vec::new(),
        }
    }
}

impl Config {
    pub fn new() -> Self {
        Self::default()
    }

    /// Set the configuration for the window.
    pub fn with_window_attributes(mut self, attrs: WindowAttributes) -> Self {
        // We need to do a swap because the window builder only takes itself as muy self
        self.window_attributes = attrs;
        self
    }

    /// Register extra font blobs (TTF/OTF/TTC bytes) into the renderer's font
    /// collection at startup. These are available synchronously on every
    /// platform, so bundled UI/app fonts render correctly without depending on
    /// the asynchronous `@font-face` `data:` URI resource fetch.
    pub fn with_fonts(mut self, font_blobs: Vec<Vec<u8>>) -> Self {
        self.font_blobs = font_blobs;
        self
    }
}
