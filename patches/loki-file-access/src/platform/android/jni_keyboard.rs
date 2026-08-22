// SPDX-License-Identifier: MIT
// Copyright (c) 2026 AppThere

//! JNI query for "is a physical keyboard usable right now?".
//!
//! Read from `Resources.getConfiguration()`, which Android keeps current as
//! keyboards attach and detach:
//!
//! - `Configuration.keyboard` — the keyboard *type*. `KEYBOARD_NOKEYS` (1) when
//!   the device has no physical keys; `KEYBOARD_QWERTY` (2) once one is
//!   attached, including over Bluetooth.
//! - `Configuration.hardKeyboardHidden` — whether that keyboard is currently
//!   *reachable*. `HARDKEYBOARDHIDDEN_YES` (2) covers a slider that is closed
//!   or a case folded shut, where the keys exist but cannot be pressed.
//!
//! Both are needed: type alone reports a folded-away keyboard as present, and
//! `hardKeyboardHidden` alone is `HARDKEYBOARDHIDDEN_NO` on devices that have no
//! keyboard at all. This mirrors the predicate Android's own
//! `InputMethodManagerService` uses to decide whether an *implicit*
//! `showSoftInput` should be honoured.
//!
//! **Requires `keyboard` in the activity's `android:configChanges`.** Without
//! it, attaching a keyboard destroys and recreates the activity instead of
//! updating the configuration in place, so this query would be answering for a
//! process that is about to be torn down.

use jni::JNIEnv;
use jni::objects::JObject;

/// `Configuration.KEYBOARD_NOKEYS` — the device has no physical keyboard.
const KEYBOARD_NOKEYS: i32 = 1;
/// `Configuration.HARDKEYBOARDHIDDEN_YES` — a physical keyboard exists but is
/// currently not reachable (slider closed, case folded).
const HARDKEYBOARDHIDDEN_YES: i32 = 2;

/// Whether a physical keyboard is attached **and** currently usable.
///
/// Returns `false` on any JNI failure. That is the safe direction: the only
/// caller uses this to *suppress* a soft-keyboard request, so a failed query
/// leaves the existing behaviour untouched rather than stranding a user with no
/// keyboard at all.
pub(super) fn has_hardware_keyboard() -> bool {
    do_query().unwrap_or(false)
}

// ── Implementation ────────────────────────────────────────────────────────────

fn do_query() -> Option<bool> {
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

fn query_with_env(env: &mut JNIEnv<'_>) -> Option<bool> {
    let ctx = ndk_context::android_context();
    // SAFETY: ndk_context stores the Application jobject; Application IS a
    // Context and provides Resources.
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

    // resources.getConfiguration() → android.content.res.Configuration
    let config = env
        .call_method(
            &resources,
            "getConfiguration",
            "()Landroid/content/res/Configuration;",
            &[],
        )
        .ok()?
        .l()
        .ok()?;

    let keyboard = env.get_field(&config, "keyboard", "I").ok()?.i().ok()?;
    let hidden = env
        .get_field(&config, "hardKeyboardHidden", "I")
        .ok()?
        .i()
        .ok()?;

    Some(keyboard != KEYBOARD_NOKEYS && hidden != HARDKEYBOARDHIDDEN_YES)
}
