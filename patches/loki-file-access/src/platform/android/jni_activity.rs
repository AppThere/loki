// SPDX-License-Identifier: MIT
// Copyright (c) 2026 AppThere

//! JNI helpers for launching `FilePickerActivity`.
//!
//! `FilePickerActivity` is a thin Java trampoline that works around the
//! `ANativeActivityCallbacks` limitation: NativeActivity has no
//! `onActivityResult` slot, but a plain `Activity` subclass does.
//!
//! Flow:
//! 1. NativeActivity calls `startActivity(Intent → FilePickerActivity)`.
//! 2. `FilePickerActivity.onCreate` calls `startActivityForResult(ACTION_OPEN_*)`.
//! 3. `FilePickerActivity.onActivityResult` receives the URI and calls
//!    `nativeOnResult(uri)` — the pre-compiled JNI hook in the Rust binary.
//! 4. The Rust future resolves.

use super::jni_common::{activity_jobject, attach_err, jvm_err, platform_err};
use crate::api::{PickOptions, SaveOptions};
use crate::error::PickerError;

// Java class coordinates for FilePickerActivity.
const FPA_PACKAGE: &str = "io.github.appthere.lokifileaccess";
const FPA_CLASS: &str = "io.github.appthere.lokifileaccess.FilePickerActivity";

// ── Public entry points ───────────────────────────────────────────────────────

/// Start `FilePickerActivity` to open one or more files.
pub(super) fn fire_open_file_picker(
    options: &PickOptions,
    allow_multiple: bool,
) -> Result<(), PickerError> {
    let ctx = ndk_context::android_context();
    let vm = unsafe { jni::JavaVM::from_raw(ctx.vm().cast()) }.map_err(jvm_err)?;
    let mut env = vm.attach_current_thread().map_err(attach_err)?;

    let intent = build_fpa_intent(&mut env, "OPEN")?;

    let mimes = options.mime_types.join(",");
    put_string_extra(&mut env, &intent, "mime_types", &mimes)?;

    // putExtra(String, boolean) — JNI signature: (Ljava/lang/String;Z)
    let key = env
        .new_string("allow_multiple")
        .map_err(|e| platform_err("allow_multiple key", e))?;
    env.call_method(
        &intent,
        "putExtra",
        "(Ljava/lang/String;Z)Landroid/content/Intent;",
        &[
            jni::objects::JValueGen::Object(&key),
            jni::objects::JValueGen::Bool(u8::from(allow_multiple)),
        ],
    )
    .map_err(|e| platform_err("putExtra allow_multiple", e))?;

    start_activity(&mut env, &intent)
}

/// Start `FilePickerActivity` to create/save a file.
pub(super) fn fire_create_file_picker(
    options: &SaveOptions,
) -> Result<(), PickerError> {
    let ctx = ndk_context::android_context();
    let vm = unsafe { jni::JavaVM::from_raw(ctx.vm().cast()) }.map_err(jvm_err)?;
    let mut env = vm.attach_current_thread().map_err(attach_err)?;

    let intent = build_fpa_intent(&mut env, "CREATE")?;

    if let Some(ref mime) = options.mime_type {
        put_string_extra(&mut env, &intent, "mime_type", mime)?;
    }
    if let Some(ref name) = options.suggested_name {
        put_string_extra(&mut env, &intent, "suggested_name", name)?;
    }

    start_activity(&mut env, &intent)
}

// ── Private helpers ───────────────────────────────────────────────────────────

/// Build an explicit `Intent` targeting `FilePickerActivity` with a `mode` extra.
fn build_fpa_intent<'a>(
    env: &mut jni::JNIEnv<'a>,
    mode: &str,
) -> Result<jni::objects::JObject<'a>, PickerError> {
    // new Intent()
    let intent_cls = env
        .find_class("android/content/Intent")
        .map_err(|e| platform_err("Intent class", e))?;
    let intent = env
        .new_object(&intent_cls, "()V", &[])
        .map_err(|e| platform_err("Intent()", e))?;

    // new ComponentName(String pkg, String cls)
    let cn_cls = env
        .find_class("android/content/ComponentName")
        .map_err(|e| platform_err("ComponentName class", e))?;
    let pkg = env
        .new_string(FPA_PACKAGE)
        .map_err(|e| platform_err("fpa package", e))?;
    let cls = env
        .new_string(FPA_CLASS)
        .map_err(|e| platform_err("fpa class", e))?;
    let component = env
        .new_object(
            &cn_cls,
            "(Ljava/lang/String;Ljava/lang/String;)V",
            &[
                jni::objects::JValueGen::Object(&pkg),
                jni::objects::JValueGen::Object(&cls),
            ],
        )
        .map_err(|e| platform_err("ComponentName()", e))?;

    // intent.setComponent(component)
    env.call_method(
        &intent,
        "setComponent",
        "(Landroid/content/ComponentName;)Landroid/content/Intent;",
        &[jni::objects::JValueGen::Object(&component)],
    )
    .map_err(|e| platform_err("setComponent", e))?;

    // intent.putExtra("mode", mode)
    put_string_extra(env, &intent, "mode", mode)?;

    Ok(intent)
}

/// Add a `String` extra to an `Intent`.
fn put_string_extra(
    env: &mut jni::JNIEnv<'_>,
    intent: &jni::objects::JObject<'_>,
    key: &str,
    value: &str,
) -> Result<(), PickerError> {
    let k = env.new_string(key).map_err(|e| platform_err("extra key", e))?;
    let v = env.new_string(value).map_err(|e| platform_err("extra value", e))?;
    env.call_method(
        intent,
        "putExtra",
        "(Ljava/lang/String;Ljava/lang/String;)Landroid/content/Intent;",
        &[
            jni::objects::JValueGen::Object(&k),
            jni::objects::JValueGen::Object(&v),
        ],
    )
    .map_err(|e| platform_err("putExtra string", e))?;
    Ok(())
}

/// Call `Context.startActivity(intent)` on the NativeActivity.
///
/// NativeActivity CAN call `startActivity` — the restriction only applies to
/// *receiving* `onActivityResult`.  `FilePickerActivity` receives its own result
/// and delivers it to Rust via the pre-compiled JNI hook.
fn start_activity(
    env: &mut jni::JNIEnv<'_>,
    intent: &jni::objects::JObject<'_>,
) -> Result<(), PickerError> {
    // SAFETY: activity_jobject() returns a GlobalRef valid for the app lifetime.
    let activity = unsafe { activity_jobject() };

    let result = env.call_method(
        &activity,
        "startActivity",
        "(Landroid/content/Intent;)V",
        &[jni::objects::JValueGen::Object(intent)],
    );

    if result.is_err() {
        // Clear any pending JNI exception (e.g. ActivityNotFoundException)
        // so subsequent JNI calls in this env don't trigger an ART abort.
        let _ = env.exception_clear();
    }

    result.map(|_| ()).map_err(|e| platform_err("startActivity", e))
}
