// SPDX-License-Identifier: MIT
// Copyright (c) 2026 AppThere

//! Android file-picker implementation using the Storage Access Framework (SAF).
//!
//! File access is mediated through content URIs and `ContentResolver`,
//! ensuring that the app never accesses files via filesystem paths (unreliable
//! on modern Android).
//!
//! # Persistence
//!
//! After the user selects a file, `ContentResolver.takePersistableUriPermission()`
//! is called with READ | WRITE flags.  This ensures the URI grant survives app
//! restarts and device reboots.
//!
//! # NativeActivity integration
//!
//! Call [`init_android`] from `android_main` before launching Dioxus:
//!
//! ```rust,no_run
//! fn android_main(android_app: android_activity::AndroidApp) {
//!     // SAFETY: activity_as_ptr() is a GlobalRef valid for app lifetime.
//!     unsafe { loki_file_access::init_android(android_app.activity_as_ptr()); }
//!     blitz_shell::set_android_app(android_app);
//!     dioxus::launch(App);
//! }
//! ```
//!
//! # Result delivery
//!
//! `ANativeActivityCallbacks` has no `onActivityResult` slot, so results cannot
//! be delivered directly to NativeActivity.  Instead, NativeActivity calls
//! `startActivity(Intent → FilePickerActivity)`, which is a transparent Java
//! trampoline that runs its own `startActivityForResult(ACTION_OPEN_DOCUMENT)`.
//! `FilePickerActivity.onActivityResult` delivers the URI via the pre-compiled
//! JNI hook `Java_io_github_appthere_lokifileaccess_FilePickerActivity_nativeOnResult`.
//!
//! **Prerequisite**: `FilePickerActivity` must be declared in `AndroidManifest.xml`
//! and its compiled `classes.dex` injected into the APK (see `scripts/build-android.ps1`).

mod jni_activity;
mod jni_common;
mod jni_fd;
mod jni_intents;

use std::sync::{Arc, Mutex, OnceLock};

use crate::api::{PickOptions, SaveOptions};
use crate::error::{AccessError, PickerError};
use crate::future::{deliver, new_pick_future};
use crate::token::{FileAccessToken, PermissionStatus, ReadSeek, TokenInner, WriteSeek};

/// Pending pick state shared between the intent launcher and the JNI callback.
static PENDING_PICK: OnceLock<
    Mutex<Option<Arc<Mutex<crate::future::PickState<Option<String>>>>>>,
> = OnceLock::new();

fn pending_pick(
) -> &'static Mutex<Option<Arc<Mutex<crate::future::PickState<Option<String>>>>>> {
    PENDING_PICK.get_or_init(|| Mutex::new(None))
}

// ── Public Android initialisation ─────────────────────────────────────────────

/// Initialise the file-access layer with the NativeActivity Java object.
///
/// Must be called from `android_main` before launching Dioxus.  Pass the
/// value returned by `android_activity::AndroidApp::activity_as_ptr()`.
///
/// # Safety
///
/// `activity_as_ptr` must be the raw `jobject` (GlobalRef) returned by
/// `AndroidApp::activity_as_ptr()`.  The pointer is valid for the lifetime of
/// the `AndroidApp`, which must outlive all file-picker calls.
pub unsafe fn init_android(activity_as_ptr: *mut std::ffi::c_void) {
    jni_common::store_activity_ptr(activity_as_ptr);
}

// ── JNI result callback (called from Java FilePickerActivity) ─────────────────

/// Delivers the selected URI (or `null` for cancellation) to the pending Rust future.
///
/// Called by `FilePickerActivity.onActivityResult` via JNI.  The method is
/// declared `private native void nativeOnResult(String)` in the Java class
/// `io.github.appthere.lokifileaccess.FilePickerActivity`, so the JVM resolves
/// it to this exported symbol automatically once the native library is loaded.
///
/// **The native library is loaded by NativeActivity before `FilePickerActivity`
/// is ever created**, so `System.loadLibrary` is not needed in the Java class.
///
/// # Safety
///
/// Must be called from a JNI-attached Java thread.
#[allow(non_snake_case)]
#[unsafe(no_mangle)]
pub unsafe extern "system" fn Java_io_github_appthere_lokifileaccess_FilePickerActivity_nativeOnResult(
    mut env: jni::JNIEnv<'_>,
    _this: jni::objects::JObject<'_>,
    uri: jni::objects::JString<'_>,
) {
    let uri_str: Option<String> = if uri.is_null() {
        None
    } else {
        env.get_string(&uri).ok().map(|s| s.into())
    };
    on_activity_result(uri_str);
}

// ── Pick / save entry points ──────────────────────────────────────────────────

/// Pick a single file for reading via `ACTION_OPEN_DOCUMENT`.
pub(crate) async fn pick_open_single(
    options: PickOptions,
) -> Result<Option<FileAccessToken>, PickerError> {
    let uri = launch_open_intent(&options, false).await?;
    match uri {
        None => Ok(None),
        Some(uri_str) => {
            jni_intents::take_persistable_uri_permission(&uri_str)?;
            let display_name =
                uri_str.rsplit('/').next().unwrap_or("unnamed").to_owned();
            Ok(Some(FileAccessToken {
                inner: TokenInner::Android {
                    uri: uri_str,
                    display_name,
                    mime_type: None,
                },
            }))
        }
    }
}

/// Pick multiple files for reading via `ACTION_OPEN_DOCUMENT`.
pub(crate) async fn pick_open_multi(
    options: PickOptions,
) -> Result<Vec<FileAccessToken>, PickerError> {
    let uri = launch_open_intent(&options, true).await?;
    match uri {
        None => Ok(vec![]),
        Some(uri_str) => {
            jni_intents::take_persistable_uri_permission(&uri_str)?;
            let display_name =
                uri_str.rsplit('/').next().unwrap_or("unnamed").to_owned();
            Ok(vec![FileAccessToken {
                inner: TokenInner::Android {
                    uri: uri_str,
                    display_name,
                    mime_type: None,
                },
            }])
        }
    }
}

/// Pick a save location via `ACTION_CREATE_DOCUMENT`.
pub(crate) async fn pick_save(
    options: SaveOptions,
) -> Result<Option<FileAccessToken>, PickerError> {
    let uri = launch_create_intent(&options).await?;
    match uri {
        None => Ok(None),
        Some(uri_str) => {
            jni_intents::take_persistable_uri_permission(&uri_str)?;
            let display_name = options
                .suggested_name
                .clone()
                .unwrap_or_else(|| "untitled".into());
            Ok(Some(FileAccessToken {
                inner: TokenInner::Android {
                    uri: uri_str,
                    display_name,
                    mime_type: options.mime_type.clone(),
                },
            }))
        }
    }
}

/// Open a content URI for reading.
pub(crate) fn open_read(inner: &TokenInner) -> Result<Box<dyn ReadSeek>, AccessError> {
    match inner {
        TokenInner::Android { uri, .. } => {
            let fd = jni_fd::open_fd(uri, "r")?;
            // SAFETY: `open_fd` returns a valid file descriptor from
            // Android's `ContentResolver.openFileDescriptor` after detaching it.
            // The caller takes ownership; it must not be double-closed.
            let file: std::fs::File = unsafe { std::os::fd::FromRawFd::from_raw_fd(fd) };
            Ok(Box::new(file))
        }
        _ => Err(AccessError::Platform {
            message: "non-Android token on Android platform".into(),
        }),
    }
}

/// Open a content URI for writing.
pub(crate) fn open_write(inner: &TokenInner) -> Result<Box<dyn WriteSeek>, AccessError> {
    match inner {
        TokenInner::Android { uri, .. } => {
            let fd = jni_fd::open_fd(uri, "w")?;
            // SAFETY: Same invariant as `open_read` — see above.
            let file: std::fs::File = unsafe { std::os::fd::FromRawFd::from_raw_fd(fd) };
            Ok(Box::new(file))
        }
        _ => Err(AccessError::Platform {
            message: "non-Android token on Android platform".into(),
        }),
    }
}

/// Check whether a persistable URI permission is still held.
pub(crate) fn check_permission(inner: &TokenInner) -> PermissionStatus {
    match inner {
        TokenInner::Android { uri, .. } => jni_fd::check_persisted_permission(uri)
            .unwrap_or(PermissionStatus::Unknown),
        _ => PermissionStatus::Unknown,
    }
}

/// Deliver the selected URI (or `None` for cancellation) to the pending future.
pub fn on_activity_result(uri: Option<String>) {
    let guard = match pending_pick().lock() {
        Ok(g) => g,
        Err(poisoned) => poisoned.into_inner(),
    };
    if let Some(ref state) = *guard {
        deliver(state, uri);
    }
}

// ── Internal helpers ──────────────────────────────────────────────────────────

fn store_pending(
    state: Arc<Mutex<crate::future::PickState<Option<String>>>>,
) -> Result<(), PickerError> {
    let mut guard = pending_pick().lock().map_err(|e| PickerError::Internal {
        message: e.to_string(),
    })?;
    *guard = Some(state);
    Ok(())
}

/// Launch `FilePickerActivity` to open a file and await the result.
async fn launch_open_intent(
    options: &PickOptions,
    allow_multiple: bool,
) -> Result<Option<String>, PickerError> {
    let (future, state) = new_pick_future::<Option<String>>();
    store_pending(state)?;
    jni_activity::fire_open_file_picker(options, allow_multiple)?;
    Ok(future.await)
}

/// Launch `FilePickerActivity` to save a file and await the result.
async fn launch_create_intent(
    options: &SaveOptions,
) -> Result<Option<String>, PickerError> {
    let (future, state) = new_pick_future::<Option<String>>();
    store_pending(state)?;
    jni_activity::fire_create_file_picker(options)?;
    Ok(future.await)
}
