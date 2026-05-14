// SPDX-License-Identifier: Apache-2.0

//! # loki-i18n
//!
//! Fluent-based internationalisation for the Loki suite.
//!
//! ## Usage
//!
//! Initialise once at app startup:
//! ```rust,no_run
//! loki_i18n::init();
//! ```
//!
//! Then translate strings with the [`fl!`] macro:
//! ```rust
//! # loki_i18n::init();
//! let label = loki_i18n::fl!("shell-home-tab");
//! let page  = loki_i18n::fl!("editor-page-label", current = 1_i64, total = 14_i64);
//! ```
//!
//! ## Locale resolution
//!
//! The active locale is detected from the OS at startup via `sys-locale`.
//! Falls back to `en-US` when the system locale has no embedded translations.

#![forbid(unsafe_code)]

mod embed;
mod loader;

use std::sync::OnceLock;

use loader::LokiBundle;

/// Re-exported so the [`fl!`] macro can reference `$crate::fluent::FluentArgs`
/// without requiring callers to add `fluent` as a direct dependency.
#[doc(hidden)]
pub use fluent;

static BUNDLE: OnceLock<LokiBundle> = OnceLock::new();

/// Initialises the global Fluent bundle using the system locale.
///
/// Must be called once before any [`fl!`] invocation. Subsequent calls are
/// no-ops — [`OnceLock`] guarantees single initialisation.
pub fn init() {
    BUNDLE.get_or_init(|| {
        let locale = sys_locale::get_locale().unwrap_or_else(|| "en-US".to_string());
        LokiBundle::load(&locale)
    });
}

/// Returns the global bundle, initialising with `en-US` if [`init`] was not
/// called first.
///
/// Calling [`init`] at app startup is preferred so locale detection runs
/// before the first UI render.
pub fn bundle() -> &'static LokiBundle {
    BUNDLE.get_or_init(|| LokiBundle::load("en-US"))
}

/// Translates a Fluent message key to the active-locale string.
///
/// # Simple message (no arguments)
/// ```rust
/// # loki_i18n::init();
/// let s = loki_i18n::fl!("shell-home-tab");
/// ```
///
/// # Message with named arguments
/// ```rust
/// # loki_i18n::init();
/// let s = loki_i18n::fl!("editor-page-label", current = 3_i64, total = 12_i64);
/// ```
///
/// Integer arguments must be `i64`; float arguments `f64`; text `&str` or
/// `String`.  If the key is not found, the key string itself is returned so
/// missing translations are visible during development.
#[macro_export]
macro_rules! fl {
    // No arguments
    ($key:expr) => {
        $crate::bundle().get($key, None)
    };
    // With key=value argument pairs
    ($key:expr, $($arg_name:ident = $arg_val:expr),+ $(,)?) => {{
        let mut args = $crate::fluent::FluentArgs::new();
        $(
            args.set(stringify!($arg_name), $arg_val);
        )+
        $crate::bundle().get($key, Some(&args))
    }};
}
