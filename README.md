# Loki

Loki is a high-performance, open-source office suite designed for both desktop and mobile users. It provides robust support for OOXML (DOCX, XLSX, etc.) and ODF (ODT, ODS, etc.) document formats, ensuring seamless cross-platform compatibility and rendering accuracy.

Written entirely in **Rust**, Loki prioritizes memory safety, blazing-fast performance, and a unified core architecture that scales beautifully from powerful desktop workstations to battery-constrained mobile devices.

## Features
- **Cross-Platform**: First-class support for Windows, macOS, Linux, iOS, and Android.
- **Format Compatibility**: Comprehensive read/write capabilities for standard office document formats (OOXML and ODF).
- **High Performance**: Powered by a unified Rust core for memory-safe, concurrent, and highly optimized document processing.
- **Open Source**: Built transparently for the community.

## Building Loki

Loki is built with Rust and Cargo. Make sure you have the [Rust toolchain](https://rustup.rs/) installed before proceeding.

### Desktop (Windows, macOS, Linux)
To build Loki for your current desktop operating system:

```bash
# Clone the repository
git clone https://github.com/AppThere/loki.git
cd loki

# Build the project
cargo build --release

# Run the project
cargo run --release
```
*Note: Depending on your OS, you may need additional build dependencies (like `build-essential`, `pkg-config`, or GTK/WebKit-related libraries on Linux).*

### Mobile (iOS and Android)

#### iOS
Requires a macOS host with Xcode installed.

```bash
# Add iOS targets
rustup target add aarch64-apple-ios x86_64-apple-ios aarch64-apple-ios-sim

# Build the core iOS library
cargo build --target aarch64-apple-ios --release
```
*(To build the Dioxus mobile frontend, you can use `dx build --platform ios`)*

#### Android
Requires the Android SDK and NDK (usually installed via Android Studio).

```bash
# Add Android targets
rustup target add aarch64-linux-android armv7-linux-androideabi i686-linux-android x86_64-linux-android

# Build the core Android library
cargo build --target aarch64-linux-android --release
```
*(To build the Dioxus mobile frontend, you can use `dx build --platform android`)*

## AI Coding Assistants & Code-Review-Graph

Loki utilizes a sophisticated knowledge graph to assist with code exploration and review. **AI coding assistants must use the `code-review-graph` MCP tools** before falling back to traditional file scanning methods (like grep, glob, or reading entire files). 

The graph is faster, cheaper, and provides essential structural context such as callers, dependents, and test coverage.

**Key Guidelines for AI Assistants:**
1. **Always Use Graph Tools First**: Use `semantic_search_nodes` or `query_graph` to explore the codebase.
2. **Understand Impact**: Use `get_impact_radius` and `get_affected_flows` instead of manually tracing imports.
3. **Review Code**: Use `detect_changes` and `get_review_context` for risk-scored analysis during code reviews.
4. **Architecture Queries**: Use `get_architecture_overview` and `list_communities` to understand high-level structure.
5. **Fallback Only**: Only fall back to Grep/Glob/Read when the graph does not cover the necessary information.

For more detailed information on tool selection and workflow, refer to the `AGENTS.md` and `GEMINI.md` files in the repository root.

## License

Loki is open source software.
