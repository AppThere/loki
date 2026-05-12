// SPDX-License-Identifier: Apache-2.0

//! AppThere shell component primitives.
//!
//! All components are application-agnostic — they must not reference any
//! application-specific route enum, document model, or business logic.

pub mod document_tab;
pub mod home_tab;
pub mod platform;
pub mod ribbon;
pub mod status_bar;
pub mod tab_bar;
pub mod title_bar;

pub use document_tab::{AtDocumentTab, AtDocumentTabProps};
pub use home_tab::{AtHomeTab, AtHomeTabProps, BuiltinTemplate, RecentDocument};
pub use platform::Platform;
pub use ribbon::{AtRibbon, AtRibbonGroup, AtRibbonGroupProps, RibbonTabDesc, RibbonTabIndex};
pub use status_bar::{AtStatusBar, AtStatusBarProps};
pub use tab_bar::{AtDocumentTabData, AtTabBar, AtTabBarProps};
pub use title_bar::{AtTitleBar, AtTitleBarProps};
