#!/usr/bin/env bash
# SPDX-License-Identifier: Apache-2.0
#
# Full Android build pipeline for Loki Text including FilePickerActivity.
#
# Steps:
#   1. Compile FilePickerActivity.java → classes.dex (javac + d8)
#   2. cargo apk build --package loki-text
#   3. Replace auto-generated AndroidManifest.xml with the custom one
#      (adds FilePickerActivity + android:hasCode="true")
#   4. Inject classes.dex into the APK
#   5. Zipalign and sign with the debug keystore
#   6. Optionally install via adb
#
# Usage:
#   ./scripts/build-android.sh [--release] [--install] [--skip-cargo-apk]
#                              [--abi auto|arm64|x64|all]
#
#   --abi auto   (default) On --install, detect the connected device's ABI and
#                build only that target (fast).  Otherwise build all ABIs.
#   --abi arm64  Build only aarch64-linux-android (arm64-v8a).
#   --abi x64    Build only x86_64-linux-android   (x86_64; Chromebooks/ARC).
#   --abi all    Build a universal multi-ABI APK (both targets from Cargo.toml's
#                build_targets) — useful for distributing one sideloadable APK.
#
#   --gpu        Enable the real Vello GPU renderer (RUSTFLAGS='--cfg android_gpu').
#                Requires a Vulkan-capable device; omit for the SwiftShader
#                emulator, which lacks the compute-shader support Vello needs.
#
# Environment variables (auto-detected if not set):
#   ANDROID_HOME / ANDROID_SDK_ROOT   Android SDK root
#   ANDROID_NDK_ROOT                  Android NDK root
#   JAVA_HOME                         JDK root (javac must be in $JAVA_HOME/bin)

set -euo pipefail

# ── Argument parsing ──────────────────────────────────────────────────────────

RELEASE=0
INSTALL=0
SKIP_CARGO_APK=0
GPU=0
ABI="auto"   # auto | arm64 | x64 | all

while [[ $# -gt 0 ]]; do
    case "$1" in
        --release)        RELEASE=1 ;;
        --install)        INSTALL=1 ;;
        --skip-cargo-apk) SKIP_CARGO_APK=1 ;;
        --gpu)            GPU=1 ;;
        --abi)            ABI="${2:-}"; shift ;;
        --abi=*)          ABI="${1#*=}" ;;
        *) echo "Unknown argument: $1" >&2; exit 1 ;;
    esac
    shift
done

# ── Resolve the cargo --target from --abi ────────────────────────────────────
# Empty CARGO_TARGET => build all ABIs listed in Cargo.toml build_targets
# (universal APK).  A non-empty value passes a single `--target` to cargo apk.

detect_device_abi() {
    # Map the connected device's reported ABI to a Rust target triple.
    local abi
    abi="$(adb shell getprop ro.product.cpu.abi 2>/dev/null | tr -d '\r')"
    case "$abi" in
        arm64-v8a)   echo "aarch64-linux-android" ;;
        x86_64)      echo "x86_64-linux-android" ;;
        armeabi*)    echo "armv7-linux-androideabi" ;;
        x86)         echo "i686-linux-android" ;;
        *)           echo "" ;;
    esac
}

CARGO_TARGET=""
case "$ABI" in
    arm64) CARGO_TARGET="aarch64-linux-android" ;;
    x64)   CARGO_TARGET="x86_64-linux-android" ;;
    all)   CARGO_TARGET="" ;;
    auto)
        if [[ "$INSTALL" -eq 1 ]]; then
            CARGO_TARGET="$(detect_device_abi)"
            if [[ -n "$CARGO_TARGET" ]]; then
                echo "==> Auto-detected device ABI -> $CARGO_TARGET"
            else
                echo "==> No device ABI detected; building all ABIs (universal APK)"
            fi
        fi
        ;;
    *) echo "Unknown --abi: '$ABI' (use auto|arm64|x64|all)" >&2; exit 1 ;;
esac

# ── Change to repo root ───────────────────────────────────────────────────────

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
cd "$SCRIPT_DIR/.."

# ── Detect Android SDK ────────────────────────────────────────────────────────

if [[ -z "${ANDROID_HOME:-}" && -z "${ANDROID_SDK_ROOT:-}" ]]; then
    if [[ "$(uname)" == "Darwin" ]]; then
        ANDROID_HOME="$HOME/Library/Android/sdk"
    else
        ANDROID_HOME="$HOME/Android/Sdk"
    fi
    if [[ -d "$ANDROID_HOME" ]]; then
        echo "==> Auto-detected SDK: $ANDROID_HOME"
    else
        echo "ERROR: ANDROID_HOME not set and $ANDROID_HOME not found." >&2
        exit 1
    fi
fi

SDK="${ANDROID_HOME:-$ANDROID_SDK_ROOT}"

# ── Detect Android NDK ────────────────────────────────────────────────────────

if [[ -z "${ANDROID_NDK_ROOT:-}" ]]; then
    NDK_BASE="$SDK/ndk"
    if [[ -d "$NDK_BASE" ]]; then
        ANDROID_NDK_ROOT="$(ls -1 "$NDK_BASE" | sort -rV | head -1)"
        ANDROID_NDK_ROOT="$NDK_BASE/$ANDROID_NDK_ROOT"
        echo "==> Auto-detected NDK: $ANDROID_NDK_ROOT"
        export ANDROID_NDK_ROOT
    else
        echo "ERROR: ANDROID_NDK_ROOT not set and $NDK_BASE not found." >&2
        exit 1
    fi
fi

# ── Detect build tools ────────────────────────────────────────────────────────

BT_BASE="$SDK/build-tools"
if [[ ! -d "$BT_BASE" ]]; then
    echo "ERROR: $BT_BASE not found — install Android build-tools via SDK Manager." >&2
    exit 1
fi
BT_VER="$(ls -1 "$BT_BASE" | sort -rV | head -1)"
BT="$BT_BASE/$BT_VER"

AAPT="$BT/aapt"
D8="$BT/d8"
ZIPALIGN="$BT/zipalign"
APKSIGNER="$BT/apksigner"

for tool in "$AAPT" "$D8" "$ZIPALIGN" "$APKSIGNER"; do
    if [[ ! -x "$tool" ]]; then
        echo "ERROR: Required build tool not found: $tool" >&2
        exit 1
    fi
done

# ── Detect android.jar ────────────────────────────────────────────────────────

PLATFORM="$(ls -1d "$SDK/platforms/android-"* 2>/dev/null | sort -rV | head -1)/android.jar"
if [[ ! -f "$PLATFORM" ]]; then
    echo "ERROR: android.jar not found — install an Android platform via SDK Manager." >&2
    exit 1
fi

# ── Detect javac ─────────────────────────────────────────────────────────────

if [[ -n "${JAVA_HOME:-}" && -x "$JAVA_HOME/bin/javac" ]]; then
    JAVAC="$JAVA_HOME/bin/javac"
elif [[ "$(uname)" == "Darwin" ]]; then
    # Android Studio bundled JDK on macOS
    AS_JAVAC="/Applications/Android Studio.app/Contents/jbr/Contents/Home/bin/javac"
    if [[ -x "$AS_JAVAC" ]]; then
        JAVAC="$AS_JAVAC"
    elif command -v javac &>/dev/null; then
        JAVAC="$(command -v javac)"
    else
        echo "ERROR: javac not found. Install JDK or Android Studio." >&2
        exit 1
    fi
else
    # Linux: check common Android Studio locations, then fall back to PATH
    for candidate in \
        "$HOME/android-studio/jbr/bin/javac" \
        "/opt/android-studio/jbr/bin/javac" \
        "/usr/local/android-studio/jbr/bin/javac"
    do
        if [[ -x "$candidate" ]]; then
            JAVAC="$candidate"
            break
        fi
    done
    if [[ -z "${JAVAC:-}" ]]; then
        if command -v javac &>/dev/null; then
            JAVAC="$(command -v javac)"
        else
            echo "ERROR: javac not found. Install JDK or Android Studio." >&2
            exit 1
        fi
    fi
fi

DEBUG_KEY="$HOME/.android/debug.keystore"

echo "==> Build tools: $BT"
echo "==> Platform:    $PLATFORM"
echo "==> javac:       $JAVAC"

# ── Paths ─────────────────────────────────────────────────────────────────────

PROFILE="$([ "$RELEASE" -eq 1 ] && echo "release" || echo "debug")"
JAVA_SRC="patches/loki-file-access/android/FilePickerActivity.java"
MANIFEST_XML="loki-text/AndroidManifest.xml"
OUT_DIR="target/android-pkg"
APK_SRC="$(pwd)/target/$PROFILE/apk/loki_text.apk"

mkdir -p "$OUT_DIR"
OUT_DIR="$(cd "$OUT_DIR" && pwd)"   # make absolute for aapt

# ── Step 1: Compile FilePickerActivity.java → classes.dex ────────────────────

echo ""
echo "==> Compiling FilePickerActivity.java..."
CLASSES_DIR="$OUT_DIR/java-classes"
DEX_DIR="$OUT_DIR/dex-out"
mkdir -p "$CLASSES_DIR" "$DEX_DIR"

"$JAVAC" -source 8 -target 8 -classpath "$PLATFORM" -d "$CLASSES_DIR" "$JAVA_SRC"

CLASS_FILE="$CLASSES_DIR/io/github/appthere/lokifileaccess/FilePickerActivity.class"
"$D8" "$CLASS_FILE" --output "$DEX_DIR" --min-api 26

DEX_PATH="$DEX_DIR/classes.dex"
echo "    DEX: $DEX_PATH"

# ── Step 2: cargo apk build ───────────────────────────────────────────────────

if [[ "$SKIP_CARGO_APK" -eq 0 ]]; then
    echo ""
    GPU_LABEL=""; [[ "$GPU" -eq 1 ]] && GPU_LABEL=", gpu"
    echo "==> cargo apk build ($PROFILE${CARGO_TARGET:+, $CARGO_TARGET}${GPU_LABEL})..."
    # Enable the Vello GPU renderer by adding the android_gpu cfg, preserving any
    # RUSTFLAGS the caller already set.
    if [[ "$GPU" -eq 1 && " ${RUSTFLAGS:-} " != *" --cfg android_gpu "* ]]; then
        export RUSTFLAGS="${RUSTFLAGS:-} --cfg android_gpu"
        echo "    GPU renderer enabled (RUSTFLAGS:${RUSTFLAGS})"
    fi
    BUILD_ARGS=(apk build --package loki-text)
    [[ "$RELEASE" -eq 1 ]] && BUILD_ARGS+=(--release)
    [[ -n "$CARGO_TARGET" ]] && BUILD_ARGS+=(--target "$CARGO_TARGET")
    # cargo-apk may exit non-zero due to a Bin/Cdylib artifact-check panic even
    # when the APK was built successfully — check for the file directly.
    cargo "${BUILD_ARGS[@]}" || true
    if [[ ! -f "$APK_SRC" ]]; then
        echo "ERROR: cargo apk build failed and APK not found at $APK_SRC" >&2
        exit 1
    fi
fi

if [[ ! -f "$APK_SRC" ]]; then
    echo "ERROR: APK not found at $APK_SRC — run without --skip-cargo-apk first." >&2
    exit 1
fi

# ── Step 3: Generate binary AndroidManifest.xml via aapt ─────────────────────

echo ""
echo "==> Packaging custom AndroidManifest.xml → binary AXML..."
MANIFEST_APK="$OUT_DIR/manifest-only.apk"
MANIFEST_EXTRACT="$OUT_DIR/manifest-extract"
mkdir -p "$MANIFEST_EXTRACT"

"$AAPT" package -f -F "$MANIFEST_APK" -M "$MANIFEST_XML" -I "$PLATFORM"

unzip -o "$MANIFEST_APK" AndroidManifest.xml -d "$MANIFEST_EXTRACT"
if [[ ! -f "$MANIFEST_EXTRACT/AndroidManifest.xml" ]]; then
    echo "ERROR: Binary manifest not found in aapt output." >&2
    exit 1
fi

# ── Step 4: Patch the cargo-apk APK ──────────────────────────────────────────

echo ""
echo "==> Patching APK (replace manifest + inject DEX)..."
APK_WORK="$OUT_DIR/loki-patched.apk"
cp "$APK_SRC" "$APK_WORK"

# Replace binary AndroidManifest.xml
(cd "$MANIFEST_EXTRACT" && "$AAPT" remove "$APK_WORK" AndroidManifest.xml 2>/dev/null || true)
(cd "$MANIFEST_EXTRACT" && "$AAPT" add "$APK_WORK" AndroidManifest.xml)

# Inject classes.dex
(cd "$DEX_DIR" && "$AAPT" add "$APK_WORK" classes.dex)

# ── Step 5: Zipalign ─────────────────────────────────────────────────────────

echo ""
echo "==> Zipaligning..."
APK_ALIGNED="$OUT_DIR/loki-aligned.apk"
rm -f "$APK_ALIGNED"
"$ZIPALIGN" -f 4 "$APK_WORK" "$APK_ALIGNED"

# ── Step 6: Sign ─────────────────────────────────────────────────────────────

echo ""
echo "==> Signing with debug keystore..."
"$APKSIGNER" sign \
    --ks "$DEBUG_KEY" \
    --ks-key-alias androiddebugkey \
    --ks-pass pass:android \
    --key-pass pass:android \
    "$APK_ALIGNED"

echo ""
echo "==> APK ready: $APK_ALIGNED"

# ── Step 7: Install ───────────────────────────────────────────────────────────

if [[ "$INSTALL" -eq 1 ]]; then
    echo ""
    echo "==> Installing on device..."
    # -r: replace existing; -d: allow version-code downgrade (dev builds use 0).
    # A debug-signed rebuild can clash with a differently-signed prior install
    # (INSTALL_FAILED_UPDATE_INCOMPATIBLE) — fall back to uninstall + install.
    if ! adb install -r -d "$APK_ALIGNED"; then
        echo "==> Replace install failed; uninstalling com.appthere.loki and retrying..."
        adb uninstall com.appthere.loki || true
        adb install "$APK_ALIGNED"
    fi
    echo "==> Installed successfully!"
fi
