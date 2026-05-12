// SPDX-License-Identifier: Apache-2.0

//! AppThere suite shared UI components and design tokens (`appthere_ui`).
//!
//! # Structure
//!
//! * [`tokens`] — design-token constants (colors, spacing, typography, layout).
//!   Import via `use appthere_ui::tokens::*` or name individual sub-modules.
//! * [`theme`] — [`AtThemeContext`] and [`use_theme`] for injecting the active
//!   theme variant from the app root to all descendant components.
//! * [`components`] — shell component primitives shared across all AppThere
//!   suite applications (title bar, tab bar, home tab, status bar).
//!
//! # Usage
//!
//! Inject the theme context at the application root:
//! ```rust,ignore
//! provide_context(AtThemeContext::default()); // ThemeVariant::Dark
//! ```
//! Then use any shell component:
//! ```rust,ignore
//! AtStatusBar { page_label: "Page 1 of 1", .. }
//! ```

#![forbid(unsafe_code)]
#![warn(missing_docs)]

pub mod components;
pub mod theme;
pub mod tokens;

pub use components::ribbon::{AtRibbon, AtRibbonGroup, RibbonTabDesc, RibbonTabIndex};
pub use components::{
    AtDocumentTab, AtDocumentTabData, AtDocumentTabProps, AtHomeTab, AtHomeTabProps, AtStatusBar,
    AtStatusBarProps, AtTabBar, AtTabBarProps, AtTitleBar, AtTitleBarProps, BuiltinTemplate,
    Platform, RecentDocument,
};
pub use theme::{use_theme, AtThemeContext, ThemeVariant};
