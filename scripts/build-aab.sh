#!/usr/bin/env bash
# SPDX-License-Identifier: Apache-2.0
#
# Build a Play-Store Android App Bundle (.aab) for Loki Text via the committed
# Gradle wrapper in android/.
#
# Why this exists (and `dx bundle aab` does not suffice): dx generates its own
# webview/wry MainActivity + manifest, which (a) omits FilePickerActivity, (b)
# lowers minSdk to 24, and (c) crashes at launch because it never calls
# blitz_shell::set_android_app before dioxus::launch.  This wrapper reproduces
# the exact NativeActivity (lib_name=loki_text) + FilePickerActivity + minSdk 26
# configuration proven to work via cargo-apk, and lets Gradle produce the AAB.
#
# Pipeline:
#   1. cargo apk build (per selected ABI) -> target/<triple>/release/libloki_text.so
#   2. Stage .so into android/app/src/main/jniLibs/<abi>/
#   3. Stage FilePickerActivity.java into the Gradle source set
#   4. ./gradlew bundleRelease -> a signed .aab
#
# Usage:
#   ./scripts/build-aab.sh [--abi all|arm64|x64] [--gpu] [--skip-cargo]
#
#   --abi all    (default) Universal bundle: arm64-v8a + x86_64.
#   --abi arm64  arm64-v8a only.
#   --abi x64    x86_64 only.
#   --gpu        Build the Rust libs with the Vello GPU renderer
#                (RUSTFLAGS='--cfg android_gpu').
#   --skip-cargo Reuse already-built target/<triple>/release/libloki_text.so.
#
# Signing: defaults to ~/.android/debug.keystore (installable for bundletool
# testing).  For a real Play upload, export before running:
#   LOKI_KEYSTORE=/path/to/upload.jks LOKI_KEYSTORE_PASS=... \
#   LOKI_KEY_ALIAS=... LOKI_KEY_PASS=...   ./scripts/build-aab.sh
# and optionally LOKI_VERSION_CODE / LOKI_VERSION_NAME.

set -euo pipefail

# ── Argument parsing ──────────────────────────────────────────────────────────

ABI="all"
GPU=0
SKIP_CARGO=0

while [[ $# -gt 0 ]]; do
    case "$1" in
        --abi)        ABI="${2:-}"; shift ;;
        --abi=*)      ABI="${1#*=}" ;;
        --gpu)        GPU=1 ;;
        --skip-cargo) SKIP_CARGO=1 ;;
        *) echo "Unknown argument: $1" >&2; exit 1 ;;
    esac
    shift
done

# ── Repo root ─────────────────────────────────────────────────────────────────

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
cd "$SCRIPT_DIR/.."
ROOT="$(pwd)"

# ── Which ABIs (triple -> jniLibs folder) ─────────────────────────────────────

declare -a TRIPLES ABI_DIRS
case "$ABI" in
    arm64) TRIPLES=(aarch64-linux-android); ABI_DIRS=(arm64-v8a) ;;
    x64)   TRIPLES=(x86_64-linux-android);  ABI_DIRS=(x86_64) ;;
    all)   TRIPLES=(aarch64-linux-android x86_64-linux-android)
           ABI_DIRS=(arm64-v8a x86_64) ;;
    *) echo "Unknown --abi: '$ABI' (use all|arm64|x64)" >&2; exit 1 ;;
esac

# ── Detect Android SDK / NDK (for cargo-apk and Gradle) ───────────────────────

SDK="${ANDROID_HOME:-${ANDROID_SDK_ROOT:-$HOME/Android/Sdk}}"
if [[ ! -d "$SDK" ]]; then
    echo "ERROR: Android SDK not found (set ANDROID_HOME)." >&2; exit 1
fi
export ANDROID_HOME="$SDK"
export ANDROID_SDK_ROOT="$SDK"

if [[ -z "${ANDROID_NDK_ROOT:-}" ]]; then
    if [[ -d "$SDK/ndk" ]]; then
        ANDROID_NDK_ROOT="$SDK/ndk/$(ls -1 "$SDK/ndk" | sort -rV | head -1)"
        export ANDROID_NDK_ROOT
    else
        echo "ERROR: ANDROID_NDK_ROOT not set and $SDK/ndk not found." >&2; exit 1
    fi
fi

# Java for Gradle: prefer JAVA_HOME, else Android Studio's bundled JBR.
if [[ -z "${JAVA_HOME:-}" ]]; then
    for cand in "$HOME/android-studio/jbr" /opt/android-studio/jbr \
                /usr/local/android-studio/jbr; do
        [[ -x "$cand/bin/java" ]] && { export JAVA_HOME="$cand"; break; }
    done
fi

echo "==> SDK:  $ANDROID_HOME"
echo "==> NDK:  $ANDROID_NDK_ROOT"
echo "==> JAVA: ${JAVA_HOME:-<from PATH>}"
echo "==> ABIs: ${ABI_DIRS[*]}"

# Ensure a debug keystore exists for the default signing config.
DEBUG_KEY="$HOME/.android/debug.keystore"
if [[ -z "${LOKI_KEYSTORE:-}" && ! -f "$DEBUG_KEY" ]]; then
    echo "==> Generating missing debug keystore..."
    mkdir -p "$HOME/.android"
    "${JAVA_HOME:-/usr}/bin/keytool" -genkeypair -v -keystore "$DEBUG_KEY" \
        -storepass android -keypass android -alias androiddebugkey \
        -keyalg RSA -keysize 2048 -validity 10000 \
        -dname "CN=Android Debug,O=Android,C=US"
fi

# ── GPU flag ──────────────────────────────────────────────────────────────────

if [[ "$GPU" -eq 1 && " ${RUSTFLAGS:-} " != *" --cfg android_gpu "* ]]; then
    export RUSTFLAGS="${RUSTFLAGS:-} --cfg android_gpu"
    echo "==> GPU renderer enabled (RUSTFLAGS:${RUSTFLAGS})"
fi

# ── Stage native libraries ────────────────────────────────────────────────────

JNI_ROOT="$ROOT/android/app/src/main/jniLibs"
echo ""
echo "==> Staging native libraries..."
# Clear stale ABIs so the bundle ships exactly the selected set.
rm -rf "$JNI_ROOT"
for i in "${!TRIPLES[@]}"; do
    triple="${TRIPLES[$i]}"
    abidir="${ABI_DIRS[$i]}"
    so="$ROOT/target/$triple/release/libloki_text.so"

    if [[ "$SKIP_CARGO" -eq 0 ]]; then
        echo "    cargo apk build ($triple)..."
        # cargo-apk may panic on a post-build artifact check even though the
        # cdylib was produced — tolerate it and verify the .so directly.
        cargo apk build --package loki-text --release --target "$triple" || true
    fi

    if [[ ! -f "$so" ]]; then
        echo "ERROR: $so not found (build failed, or --skip-cargo with no prior build)." >&2
        exit 1
    fi

    mkdir -p "$JNI_ROOT/$abidir"
    cp "$so" "$JNI_ROOT/$abidir/libloki_text.so"
    echo "    -> jniLibs/$abidir/libloki_text.so ($(du -h "$so" | cut -f1))"
done

# ── Stage FilePickerActivity.java (single source of truth in patches/) ────────

JAVA_PKG_DIR="$ROOT/android/app/src/main/java/io/github/appthere/lokifileaccess"
echo ""
echo "==> Staging FilePickerActivity.java..."
rm -rf "$ROOT/android/app/src/main/java"
mkdir -p "$JAVA_PKG_DIR"
cp "$ROOT/patches/loki-file-access/android/FilePickerActivity.java" "$JAVA_PKG_DIR/"

# ── Gradle bundleRelease ──────────────────────────────────────────────────────

echo ""
echo "==> ./gradlew bundleRelease..."
( cd "$ROOT/android" && ./gradlew --no-daemon bundleRelease )

AAB="$ROOT/android/app/build/outputs/bundle/release/app-release.aab"
if [[ ! -f "$AAB" ]]; then
    echo "ERROR: expected AAB not found at $AAB" >&2; exit 1
fi

echo ""
echo "==> AAB ready: $AAB"
ls -la "$AAB"
