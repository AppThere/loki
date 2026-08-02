// SPDX-License-Identifier: Apache-2.0

//! AppThere shell component primitives.
//!
//! All components are application-agnostic — they must not reference any
//! application-specific route enum, document model, or business logic.

mod calibrate_dialog;
pub mod color_picker;
pub mod confirm_dialog;
pub mod document_tab;
pub mod home_tab;
pub mod icons;
pub mod infobar;
pub mod macro_security;
pub mod overlay;
pub mod panel_host;
pub mod platform;
pub mod popover;
pub mod ribbon;
pub mod status_bar;
pub mod tab_bar;
pub mod template_browser;
pub mod title_bar;
pub mod zoom;
pub(crate) mod zoom_control;

pub use calibrate_dialog::{AtCalibrateDialog, AtCalibrateLabels, REFERENCE_MM};
pub use color_picker::{
    AtColorPickerLabels, AtColorPickerPanel, AtColorPickerTrigger, AtColorSwatch,
};
pub use confirm_dialog::{AtConfirmDialog, AtConfirmDialogProps};
pub use document_tab::{AtDocumentTab, AtDocumentTabProps};
pub use home_tab::{AtHomeTab, AtHomeTabProps, BuiltinTemplate, RecentDocument};
pub use infobar::{AtInfobar, AtInfobarProps};
pub use macro_security::{
    AtMacroTrustDialog, AtMacroTrustDialogProps, AtNetworkPrompt, AtNetworkPromptProps,
    AtPermissionPrompt, AtPermissionPromptProps, MacroDialogFrame, MacroDialogFrameProps,
    MacroGrantChoice, MacroTrustChoice,
};
pub use overlay::{
    use_backdrop, use_provide_backdrop, AtBackdropContext, AtBackdropHost, BACKDROP_Z_INDEX,
};
pub use panel_host::{AtPanelHost, AtPanelHostProps, PanelPosture};
pub use platform::Platform;
pub use ribbon::{AtRibbon, AtRibbonGroup, AtRibbonGroupProps, RibbonTabDesc, RibbonTabIndex};
pub use status_bar::{AtStatusBar, AtStatusBarProps};
pub use tab_bar::{AtDocumentTabData, AtTabBar, AtTabBarProps};
pub use template_browser::{AtTemplateBrowser, AtTemplateBrowserProps};
pub use title_bar::{AtTitleBar, AtTitleBarProps};
pub use zoom::fit::{actual_size_zoom_percent, fit_page_zoom_percent, fit_width_zoom_percent};
pub use zoom::{
    clamp_zoom_percent, next_zoom, parse_zoom_percent, prev_zoom, zoom_is_capped,
    zoom_percent_to_permille, ZOOM_MAX_PERCENT, ZOOM_MIN_PERCENT, ZOOM_PRESETS_PERCENT,
};
pub use zoom_control::{zoom_rows, AtZoomControl, AtZoomLabels, ZoomCommands, ZoomRow};
