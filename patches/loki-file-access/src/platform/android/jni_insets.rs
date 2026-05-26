// SPDX-License-Identifier: MIT
// Copyright (c) 2026 AppThere

//! JNI query for Android system-bar heights.
//!
//! Uses `Resources.getDimensionPixelSize` with the well-known system resource
//! identifiers `status_bar_height` and `navigation_bar_height`. These resource
//! identifiers are available immediately at process start — before the window
//! is laid out — making them safe to query from `android_main`.
//!
//! Returns heights in density-independent pixels (dp / CSS px) so callers can
//! apply them directly as CSS `padding` values.

use jni::objects::{JObject, JValueGen};
use jni::JNIEnv;

/// Query Android system-bar heights from OS resources.
///
/// Returns `(top_dp, bottom_dp)` where:
/// - `top_dp` is the status-bar height (top of screen)
/// - `bottom_dp` is the navigation-bar height (bottom of screen; 0 when using
///   full gesture navigation with no visible bar)
///
/// Falls back to `(24.0, 0.0)` on any JNI failure so the status bar area is
/// always reserved even if the exact height cannot be determined.
pub(super) fn query_insets_dp() -> (f32, f32) {
    // Clear any stale JNI exception from the fallback path so the caller's
    // thread remains usable. Any pending exception is cleared at the end of
    // do_query regardless of the outcome.
    do_query().unwrap_or((24.0, 0.0))
}

// ── Implementation ────────────────────────────────────────────────────────────

fn do_query() -> Option<(f32, f32)> {
    let ctx = ndk_context::android_context();
    // SAFETY: ndk_context stores the JVM pointer initialised by android-activity
    // before android_main is called. It is valid for the process lifetime.
    let vm = unsafe { jni::JavaVM::from_raw(ctx.vm().cast()) }.ok()?;
    let mut env = vm.attach_current_thread().ok()?;

    let result = query_with_env(&mut env);

    // Clear any pending JNI exception so the calling thread stays usable.
    let _ = env.exception_clear();

    result
}

fn query_with_env(env: &mut JNIEnv<'_>) -> Option<(f32, f32)> {
    let ctx = ndk_context::android_context();
    // SAFETY: ndk_context stores the Application jobject; Application IS a
    // Context and provides Resources, so this is safe for resource queries.
    let context = unsafe { JObject::from_raw(ctx.context().cast()) };

    // context.getResources() → android.content.res.Resources
    let resources = env
        .call_method(
            &context,
            "getResources",
            "()Landroid/content/res/Resources;",
            &[],
        )
        .ok()?
        .l()
        .ok()?;

    // resources.getDisplayMetrics() → android.util.DisplayMetrics
    let metrics = env
        .call_method(
            &resources,
            "getDisplayMetrics",
            "()Landroid/util/DisplayMetrics;",
            &[],
        )
        .ok()?
        .l()
        .ok()?;

    // DisplayMetrics.density: float — e.g. 1.0 (mdpi), 2.0 (xhdpi), 3.0 (xxhdpi)
    let density = env.get_field(&metrics, "density", "F").ok()?.f().ok()?;
    if density <= 0.0 {
        return None;
    }

    let top_dp = dimen_dp(env, &resources, "status_bar_height", density).unwrap_or(24.0);
    let bottom_dp = dimen_dp(env, &resources, "navigation_bar_height", density).unwrap_or(0.0);

    Some((top_dp, bottom_dp))
}

/// Look up one Android system dimension resource and convert physical pixels → dp.
fn dimen_dp(
    env: &mut JNIEnv<'_>,
    resources: &JObject<'_>,
    name: &str,
    density: f32,
) -> Option<f32> {
    let name_jstr = env.new_string(name).ok()?;
    let type_jstr = env.new_string("dimen").ok()?;
    let pkg_jstr = env.new_string("android").ok()?;

    // resources.getIdentifier(name, "dimen", "android") → int resource ID
    let res_id: i32 = env
        .call_method(
            resources,
            "getIdentifier",
            "(Ljava/lang/String;Ljava/lang/String;Ljava/lang/String;)I",
            &[
                JValueGen::Object(&name_jstr),
                JValueGen::Object(&type_jstr),
                JValueGen::Object(&pkg_jstr),
            ],
        )
        .ok()?
        .i()
        .ok()?;

    if res_id == 0 {
        return Some(0.0);
    }

    // resources.getDimensionPixelSize(resId) → int physical pixels
    let px: i32 = env
        .call_method(
            resources,
            "getDimensionPixelSize",
            "(I)I",
            &[JValueGen::Int(res_id)],
        )
        .ok()?
        .i()
        .ok()?;

    Some(px as f32 / density)
}
