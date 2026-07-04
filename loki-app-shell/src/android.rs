// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 AppThere Loki contributors

//! Shared Android `NativeActivity` entry point for the Loki suite binaries.
//!
//! Each binary (`loki-text`, `loki-spreadsheet`, `loki-presentation`) is an
//! independent `cdylib` and must export its own `android_main` symbol, so the
//! entry point can't be a plain function in this crate. The bootstrap *body*,
//! however, was duplicated verbatim across all three (Spec 01 audit A-14): the
//! Android-16 double-fire guard, logger + panic-to-logcat setup, file-access
//! init, safe-area insets, `set_android_app`, i18n, and the Dioxus launch.
//!
//! [`android_main!`] generates that body once. It is a macro rather than a
//! function so the expansion uses each binary's own `dioxus` / `blitz_shell` /
//! `android_activity` dependencies — keeping this crate lean and
//! `#![forbid(unsafe_code)]` (the emitted `unsafe` lives in the *caller*, under
//! the scoped `#[allow(unsafe_code)]` the macro attaches; Spec 01 audit A-7).
//!
//! ## Usage
//!
//! ```ignore
//! // in each binary's lib.rs, with `#![deny(unsafe_code)]` at the crate root:
//! loki_app_shell::android_main!(tag = "LOKI-SHEET", root = app::App, file_access = activity_ptr);
//! ```
//!
//! `file_access = activity_ptr` passes `android_app.activity_as_ptr()` to
//! `loki_file_access::init_android`; `file_access = null_context` passes a null
//! pointer (the call is a no-op kept for API compatibility — the JNI
//! `Application` context comes from `ndk_context`, which `android-activity`
//! initialises before `android_main` runs).

/// Generates a binary's Android `android_main` FFI entry point. See the
/// [module docs](self) for usage and the `file_access` modes.
#[macro_export]
macro_rules! android_main {
    (tag = $tag:literal, root = $root:path, file_access = activity_ptr) => {
        $crate::android_main!(@impl $tag, $root, {
            // SAFETY: activity_as_ptr() is a GlobalRef owned by android_app, which
            // blitz_shell::set_android_app keeps alive for the process lifetime.
            unsafe { ::loki_file_access::init_android(android_app.activity_as_ptr()) }
        });
    };
    (tag = $tag:literal, root = $root:path, file_access = null_context) => {
        $crate::android_main!(@impl $tag, $root, {
            // init_android is a no-op kept for API compatibility; the Application
            // context used by all JNI calls comes from ndk_context, which
            // android-activity initialises before android_main is called.
            unsafe { ::loki_file_access::init_android(::core::ptr::null_mut()) }
        });
    };
    (@impl $tag:literal, $root:path, $init_file_access:block) => {
        #[cfg(target_os = "android")]
        // COMPAT(android-16): On Android 16 (API 36) ANativeActivity_onCreate fires
        // twice in rapid succession, spawning two concurrent android_main threads.
        // A static OnceLock would also block legitimate activity-recreation relaunches
        // within the same process (process reuse), so use a Mutex<bool> "is-running"
        // flag instead: set on entry, cleared on exit, so concurrent duplicates are
        // rejected while sequential re-entries (activity destroyed → recreated) succeed.
        static ANDROID_MAIN_RUNNING: ::std::sync::Mutex<bool> = ::std::sync::Mutex::new(false);

        #[cfg(target_os = "android")]
        #[unsafe(no_mangle)]
        // FFI entry point: the `#[unsafe(no_mangle)]` attribute and the
        // `init_android` call below are the only `unsafe` in this binary. The
        // crate root is `#![deny(unsafe_code)]`; this scopes the exception to the
        // entry point alone (Spec 01 audit A-7).
        #[allow(unsafe_code)]
        fn android_main(android_app: ::android_activity::AndroidApp) {
            {
                let mut running = ANDROID_MAIN_RUNNING
                    .lock()
                    .unwrap_or_else(|p| p.into_inner());
                if *running {
                    // Concurrent duplicate invocation on Android 16 — discard it.
                    return;
                }
                *running = true;
            }
            ::android_logger::init_once(
                ::android_logger::Config::default()
                    .with_tag($tag)
                    .with_max_level(::log::LevelFilter::Debug),
            );
            // Route panic messages to logcat. The default panic hook writes to
            // stderr, which Android discards — without this, any Rust panic (e.g.
            // during GPU renderer init) is indistinguishable from a native crash.
            ::std::panic::set_hook(::std::boxed::Box::new(|info| {
                ::log::error!("PANIC: {info}");
            }));
            ::log::info!("android_main: start");
            $init_file_access;
            let (top, bottom) = ::loki_file_access::query_insets_dp();
            ::log::info!("android_main: safe area insets top={top} bottom={bottom}");
            ::appthere_ui::set_safe_area_insets(::appthere_ui::SafeAreaInsets {
                top,
                bottom,
                ..::core::default::Default::default()
            });
            // Store the internal data path before android_app is moved, so that
            // recent_documents can persist to a writable location on Android.
            if let Some(data_path) = android_app.internal_data_path() {
                $crate::recent_documents::set_android_data_dir(data_path);
            }
            ::blitz_shell::set_android_app(android_app);
            ::log::info!("android_main: i18n init");
            ::loki_i18n::init();
            ::log::info!("android_main: launching dioxus");
            // Register the bundled UI + metric-compatible fonts directly into the
            // renderer's font collection at startup, so they resolve synchronously
            // on Android instead of relying on the asynchronous `@font-face`
            // `data:` URI fetch (which does not reliably run before first paint on
            // Android, leaving UI chrome digits in a wide system fallback). See
            // `loki_fonts::ui_font_blobs`.
            ::dioxus::native::launch_cfg(
                $root,
                ::std::vec![],
                ::std::vec![::std::boxed::Box::new(
                    ::dioxus::native::Config::new().with_fonts(::loki_fonts::ui_font_blobs()),
                )],
            );
            ::log::info!("android_main: dioxus exited");
            // Clear the running flag so a subsequent activity-recreation relaunch
            // (in the same process) is allowed to proceed.
            *ANDROID_MAIN_RUNNING
                .lock()
                .unwrap_or_else(|p| p.into_inner()) = false;
        }
    };
}
