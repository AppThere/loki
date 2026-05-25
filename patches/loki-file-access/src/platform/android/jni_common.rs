// SPDX-License-Identifier: MIT
// Copyright (c) 2026 AppThere

//! Shared JNI error helpers and the NativeActivity object reference.
//!
//! Extracted here so both [`super::jni_intents`] and [`super::jni_activity`]
//! can import without circular dependencies.

use std::sync::atomic::{AtomicPtr, Ordering};

use crate::error::PickerError;

// ── NativeActivity reference ──────────────────────────────────────────────────
//
// android-activity v0.6 intentionally stores the *Application* (not the
// Activity) in ndk_context, because Application outlives the Activity
// lifecycle.  `startActivity` and `startActivityForResult` are Activity-only
// methods, so we keep a separate GlobalRef to the NativeActivity here.
//
// Populated by `init_android()` from `AndroidApp::activity_as_ptr()`.

pub(super) static ACTIVITY_PTR: AtomicPtr<std::ffi::c_void> =
    AtomicPtr::new(std::ptr::null_mut());

pub(super) fn store_activity_ptr(ptr: *mut std::ffi::c_void) {
    ACTIVITY_PTR.store(ptr, Ordering::SeqCst);
}

/// Return the best available Activity `JObject`.
///
/// Prefers the NativeActivity GlobalRef stored by `init_android()`.
/// Falls back to the Application object stored in `ndk_context` (works for
/// non-NativeActivity hosts that store a real Activity there).
///
/// # Safety
///
/// The returned `JObject` is a GlobalRef (or ndk_context Application ref) —
/// both are valid for the lifetime of the process.  The caller is responsible
/// for not outliving the associated `JNIEnv` frame.
pub(super) unsafe fn activity_jobject<'local>() -> jni::objects::JObject<'local> {
    let raw = ACTIVITY_PTR.load(Ordering::SeqCst);
    if !raw.is_null() {
        // SAFETY: caller guarantees this is a valid GlobalRef (set by init_android).
        unsafe { jni::objects::JObject::from_raw(raw.cast()) }
    } else {
        let ctx = ndk_context::android_context();
        // SAFETY: ndk_context stores a valid Application/Activity jobject.
        unsafe { jni::objects::JObject::from_raw(ctx.context().cast()) }
    }
}

// ── Error conversions ─────────────────────────────────────────────────────────

pub(super) fn jvm_err(e: jni::errors::Error) -> PickerError {
    PickerError::Platform {
        message: format!("failed to get JavaVM: {e}"),
    }
}

pub(super) fn attach_err(e: jni::errors::Error) -> PickerError {
    PickerError::Platform {
        message: format!("failed to attach JNI thread: {e}"),
    }
}

pub(super) fn platform_err(ctx: &str, e: impl std::fmt::Display) -> PickerError {
    PickerError::Platform {
        message: format!("{ctx}: {e}"),
    }
}
