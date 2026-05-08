# Loki

Loki is a high-performance, open-source office suite designed for both desktop and mobile. It reads and writes OOXML (DOCX) and ODF (ODT) documents and renders them through a GPU-accelerated Vello/wgpu pipeline backed by the [Blitz](https://github.com/DioxusLabs/blitz) native renderer.

Written entirely in **Rust**, Loki targets desktop (Windows, macOS, Linux) and mobile (iOS, Android) from a single codebase.

## Architecture

| Layer | Crate | Role |
|-------|-------|------|
| Document model | `loki-doc-model` | Format-neutral AST + Loro CRDT sync |
| Import | `loki-ooxml`, `loki-odf` | DOCX / ODT → document model |
| Layout | `loki-layout` | Parley text layout, page pagination |
| Rendering | `loki-vello` | Vello scene builder (cursor, selection handles) |
| UI shell | `loki-text` | Dioxus Native app (editing, touch input, routing) |

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

`dx` must be installed and must match the `dioxus` library version in use:

```bash
cargo install dioxus-cli --version "0.7.5"
dx --version  # should print 0.7.5
```

> **Version note:** `dx serve --platform android` will fail with a
> `dioxus-desktop ^0.7.4` dependency error if the dx CLI version does not match
> the dioxus library version. Always install the same patch version.

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
min_sdk_version = 26          # wgpu requires Vulkan, available from API 26
target_sdk_version = 34

[package.metadata.android.application]
label = "Loki"
```

> **API 26 minimum:** wgpu's Vulkan backend requires Android 8.0 (API 26) or
> later. Devices below this cannot run the Blitz render pipeline.

### Build and deploy

```bash
# Connect a device via USB (enable USB debugging) or start an emulator
adb devices                            # confirm device is visible

# Build, install, and launch on the connected device
cargo apk run -p loki-text --release
```

For debug builds (faster compile, enables `dx` hot-patch channel if wired):

```bash
cargo apk run -p loki-text
```

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

Loki vendors and patches three upstream crates to work around pre-1.0 gaps.
See [`docs/patches.md`](docs/patches.md) for the full list and removal conditions.

| Patch | Reason |
|-------|--------|
| `patches/blitz-shell` | Forwards `WindowEvent::Touch` as mouse events (upstream has empty `{}` arm) |
| `patches/dioxus-native-dom` | Implements `convert_touch_data` and other `unimplemented!()` event converters |
| `patches/blitz-dom` | Fixes tabindex focus-on-click for non-input elements |
| `patches/fontique` | Fixes missing `fontconfig_sys` alias in the crates.io 0.8.0 publish |

## AI Coding Assistants

Loki uses the `code-review-graph` MCP server for token-efficient code exploration.
**Always use graph tools before grep/glob/Read.** See [`CLAUDE.md`](CLAUDE.md),
[`AGENTS.md`](AGENTS.md), and [`GEMINI.md`](GEMINI.md) for assistant-specific
instructions.

## License

Loki is open source software. See [LICENSE](LICENSE).
