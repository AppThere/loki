// SPDX-License-Identifier: MIT
// Copyright (c) 2026 AppThere

//! JNI helpers for post-pick SAF operations.
//!
//! Covers `ContentResolver.takePersistableUriPermission`, URI parsing, and the
//! content resolver accessor used by [`super::jni_fd`].  Intent launching is
//! handled by [`super::jni_activity`] (via `FilePickerActivity` trampoline).

use super::jni_common::{attach_err, jvm_err, platform_err};
use crate::error::PickerError;

// ── SAF post-pick operations ──────────────────────────────────────────────────

/// Call `ContentResolver.takePersistableUriPermission` for a content URI.
///
/// Grants READ | WRITE persistable permission so the URI survives app restarts.
pub(super) fn take_persistable_uri_permission(uri: &str) -> Result<(), PickerError> {
    let ctx = ndk_context::android_context();
    let vm = unsafe { jni::JavaVM::from_raw(ctx.vm().cast()) }.map_err(jvm_err)?;
    let mut env = vm.attach_current_thread().map_err(attach_err)?;

    let uri_obj = parse_uri(&mut env, uri)?;
    let resolver = get_content_resolver(&mut env, &ctx)?;

    // FLAG_GRANT_READ_URI_PERMISSION (1) | FLAG_GRANT_WRITE_URI_PERMISSION (2)
    env.call_method(
        &resolver,
        "takePersistableUriPermission",
        "(Landroid/net/Uri;I)V",
        &[
            jni::objects::JValueGen::Object(&uri_obj),
            jni::objects::JValueGen::Int(3),
        ],
    )
    .map_err(|e| platform_err("takePersistableUriPermission", e))?;

    Ok(())
}

// ── Shared helpers ────────────────────────────────────────────────────────────

/// Parse a URI string into a `Uri` JNI object via `Uri.parse(String)`.
pub(super) fn parse_uri<'a>(
    env: &mut jni::JNIEnv<'a>,
    uri: &str,
) -> Result<jni::objects::JObject<'a>, PickerError> {
    let cls = env
        .find_class("android/net/Uri")
        .map_err(|e| platform_err("Uri class", e))?;
    let s = env
        .new_string(uri)
        .map_err(|e| platform_err("URI string", e))?;
    env.call_static_method(
        &cls,
        "parse",
        "(Ljava/lang/String;)Landroid/net/Uri;",
        &[jni::objects::JValueGen::Object(&s)],
    )
    .map_err(|e| platform_err("Uri.parse", e))?
    .l()
    .map_err(|e| platform_err("Uri.parse object", e))
}

/// Get the `ContentResolver` from the ndk_context Application/Activity.
pub(super) fn get_content_resolver<'a>(
    env: &mut jni::JNIEnv<'a>,
    ctx: &ndk_context::AndroidContext,
) -> Result<jni::objects::JObject<'a>, PickerError> {
    // SAFETY: ndk_context stores a valid Application jobject.
    let context = unsafe { jni::objects::JObject::from_raw(ctx.context().cast()) };
    env.call_method(
        &context,
        "getContentResolver",
        "()Landroid/content/ContentResolver;",
        &[],
    )
    .map_err(|e| platform_err("getContentResolver", e))?
    .l()
    .map_err(|e| platform_err("getContentResolver object", e))
}

