// SPDX-License-Identifier: Apache-2.0

//! AppThere shell component primitives.
//!
//! All components are application-agnostic — they must not reference any
//! application-specific route enum, document model, or business logic.

pub mod confirm_dialog;
pub mod document_tab;
pub mod home_tab;
pub mod icons;
pub mod overlay;
pub mod panel_host;
pub mod platform;
pub mod ribbon;
pub mod status_bar;
pub mod tab_bar;
pub mod template_browser;
pub mod title_bar;
pub mod zoom;

pub use confirm_dialog::{AtConfirmDialog, AtConfirmDialogProps};
pub use document_tab::{AtDocumentTab, AtDocumentTabProps};
pub use home_tab::{AtHomeTab, AtHomeTabProps, BuiltinTemplate, RecentDocument};
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
pub use zoom::next_zoom;
