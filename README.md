# Loki

Loki is a high-performance, open-source office suite designed for both desktop and mobile. It reads and writes OOXML (DOCX) and ODF (ODT) documents, publishes to **PDF/X** and **EPUB 3.3**, and renders everything through a GPU-accelerated Vello/wgpu pipeline backed by the [Blitz](https://github.com/DioxusLabs/blitz) native renderer.

Written entirely in **Rust**, Loki targets desktop (Windows, macOS, Linux) and mobile (iOS, Android) from a single codebase.

## Architecture

| Layer | Crate | Role |
|-------|-------|------|
| Document model | `loki-doc-model` | Format-neutral AST, metadata, + Loro CRDT sync |
| Import | `loki-ooxml`, `loki-odf` | DOCX / XLSX / ODT / ODS → document model |
| Export (office) | `loki-ooxml`, `loki-odf` | document model → DOCX / ODT / ODS |
| Export (publish) | `loki-pdf`, `loki-epub` | PDF/X (X-1a/X-3/X-4) and EPUB 3.3 |
| Layout | `loki-layout` | Parley text layout, page pagination |
| Rendering | `loki-vello`, `loki-renderer` | Vello scene builder + tiered GPU page cache |
| UI shell | `loki-text` | Dioxus Native app (editing, touch input, routing) |
| Design system | `appthere-ui` | Shared tokens, theme, and shell components |
| Fidelity testing | `loki-acid` | ACID rendering-fidelity harness (see `loki-acid/README.md`) |

The renderer stack is **Blitz → wgpu 0.19 → Vulkan / Metal / DX12 / OpenGL ES**. There is no WebView — all rendering is GPU-native.

## Prerequisites

Install the [Rust toolchain](https://rustup.rs/) (stable, 1.86+):

```bash
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh
```

**Linux** — install system graphics libraries:

```bash
# Debian / Ubuntu
sudo apt install libvulkan-dev libxkbcommon-dev libwayland-dev \
                 pkg-config libfontconfig-dev

# Fedora
sudo dnf install vulkan-loader-devel libxkbcommon-devel wayland-devel \
                 fontconfig-devel
```

**macOS / Windows** — no additional system packages needed; wgpu uses Metal and DX12 respectively.

## Running on Desktop

```bash
git clone https://github.com/AppThere/loki.git
cd loki

# Development (hot-reload via Dioxus CLI)
dx serve --package loki-text --platform native

# Or build and run directly with Cargo
cargo run -p loki-text
```

`dx` must be installed and must match the `dioxus` library version in use.
Loki pins dioxus to an exact version (`=0.7.9`) so the vendored
`dioxus-native{,-dom}` patches apply — see
[Workspace patches](#workspace-patches) and `docs/patches.md`. Install the
matching `dx`:

```bash
cargo install dioxus-cli --version "0.7.9"
dx --version  # should print 0.7.9
```

> **Version note:** `dx serve` will fail with a dependency-mismatch error if the
> dx CLI version does not match the dioxus library version. Always install the
> same patch version, and bump both together (see "Upgrading Dioxus" in
> `docs/patches.md`).

## Running on Android

Loki uses the **Blitz/wgpu GPU renderer** (`features = ["native"]`), not the
WebView-based mobile renderer. `dx serve --platform android` is therefore the
**wrong command** — it activates the WebView renderer instead. Use
[cargo-apk](https://github.com/rust-mobile/cargo-apk) instead.

### Prerequisites

1. **Android Studio** — install the SDK and NDK (r25c or newer recommended).

2. Set environment variables:

   ```bash
   export ANDROID_HOME=$HOME/Library/Android/sdk   # macOS
   export NDK_HOME=$ANDROID_HOME/ndk/<version>
   ```

3. Add the Android Rust targets:

   ```bash
   rustup target add aarch64-linux-android          # most modern devices
   rustup target add armv7-linux-androideabi         # 32-bit devices (optional)
   ```

4. Install cargo-apk:

   ```bash
   cargo install cargo-apk
   ```

### Add Android metadata to loki-text/Cargo.toml

```toml
[package.metadata.android]
package = "com.appthere.loki"
build_targets = ["aarch64-linux-android"]

[package.metadata.android.sdk]
min_sdk_version = 26          # wgpu requires Vulkan, available from API 26
target_sdk_version = 34
compile_sdk_version = 34

[package.metadata.android.application]
label = "Loki"
```

> **API 26 minimum:** wgpu's Vulkan backend requires Android 8.0 (API 26) or
> later. Devices below this cannot run the Blitz render pipeline.

### Build and deploy

cargo-apk must be run from inside the `loki-text/` directory (or with
`--manifest-path loki-text/Cargo.toml`). The workspace root `Cargo.toml` is a
virtual manifest with no `[package]` section, which cargo-apk cannot use.

```bash
# Connect a device via USB (enable USB debugging) or start an emulator
adb devices                            # confirm device is visible

# Build, install, and launch on the connected device (aarch64 for real device)
cd loki-text
ANDROID_NDK_ROOT="$ANDROID_HOME/ndk/<version>" \
  cargo apk run --bin loki-text --target aarch64-linux-android --release
```

For the **x86_64 emulator** (debug build, faster iteration):

```bash
cd loki-text
ANDROID_NDK_ROOT="$ANDROID_HOME/ndk/<version>" \
  cargo apk build --lib --target x86_64-linux-android
# Install manually — debug APKs are large, so skip incremental protocol:
adb uninstall com.appthere.loki
adb install --no-incremental target/debug/apk/loki_text.apk
adb shell am start -n com.appthere.loki/android.app.NativeActivity
```

> **NDK note:** Set `ANDROID_NDK_ROOT` explicitly for each cargo-apk invocation;
> cargo-apk does not reliably pick it up from `ANDROID_HOME` on all platforms.
> On Windows use `$env:LOCALAPPDATA\Android\Sdk\ndk\<version>`.

> **Large APK:** Debug builds are ~800 MB. `adb install` without
> `--no-incremental` will fail with `INSTALL_FAILED_INSUFFICIENT_STORAGE`.

## Running on iOS

Requires macOS with Xcode 15+.

1. Add iOS targets:

   ```bash
   rustup target add aarch64-apple-ios          # physical device
   rustup target add aarch64-apple-ios-sim       # Apple Silicon simulator
   ```

2. Build the library:

   ```bash
   cargo build -p loki-text --target aarch64-apple-ios --release
   ```

   Packaging into an `.ipa` / Xcode project requires an iOS app harness. This
   is not yet automated — track progress in the issue tracker.

## Workspace patches

Loki vendors and patches six upstream crates to work around pre-1.0 gaps.
See [`docs/patches.md`](docs/patches.md) for the full list, removal conditions,
and the **Upgrading Dioxus** procedure (the dioxus patches are version-pinned).

| Patch | Reason |
|-------|--------|
| `patches/blitz-shell` | Forwards `WindowEvent::Touch` as mouse events (upstream has empty `{}` arm) |
| `patches/dioxus-native-dom` | Implements `convert_touch_data` and other `unimplemented!()` event converters |
| `patches/dioxus-native` | Calls `request_redraw()` after CSS head-element insertion (Android blank screen fix) |
| `patches/blitz-net` | Switches reqwest from native-tls to rustls (Android has no `libssl.so`) |
| `patches/blitz-dom` | Fixes tabindex focus-on-click for non-input elements |

## AI Coding Assistants

Loki uses the `code-review-graph` MCP server for token-efficient code exploration.
**Always use graph tools before grep/glob/Read.** See [`CLAUDE.md`](CLAUDE.md),
[`AGENTS.md`](AGENTS.md), and [`GEMINI.md`](GEMINI.md) for assistant-specific
instructions.

## License

Loki is open source software. See [LICENSE](LICENSE).
