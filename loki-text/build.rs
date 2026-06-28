// SPDX-License-Identifier: Apache-2.0

//! Build script for loki-text.
//!
//! Creates a symlink from `target/{profile}/assets/` pointing at
//! `loki-text/assets/` so the Dioxus Native asset resolver can serve font
//! files (and future assets) via `dioxus://` URLs at runtime during
//! development and CI builds.
//!
//! # How the Dioxus Native asset resolver works
//!
//! `dioxus_asset_resolver::native::serve_asset` resolves `dioxus://` URLs by
//! looking for files relative to the running executable's parent directory:
//!
//!   URL: `dioxus:///assets/fonts/foo.ttf`
//!   → path checked: `<exe_dir>/assets/fonts/foo.ttf`
//!   → in a debug build: `target/debug/assets/fonts/foo.ttf`
//!
//! The symlink created here makes that path valid for all dev and CI builds
//! without copying files on every build.
//!
//! # Production bundles
//!
//! When packaging a release bundle, the bundler (e.g. `dx bundle`) is
//! expected to copy `loki-text/assets/` into the bundle's asset directory.
//! The symlink is not needed in that case.

// Build script, not library runtime: a panic fails the build, which is the
// intended behaviour for a missing `OUT_DIR`/`CARGO_MANIFEST_DIR`.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use std::env;
#[cfg(not(unix))]
use std::fs;
use std::path::PathBuf;

fn main() {
    let manifest_dir = PathBuf::from(env::var("CARGO_MANIFEST_DIR").unwrap());
    let out_dir = PathBuf::from(env::var("OUT_DIR").unwrap());

    // Navigate from OUT_DIR (target/{profile}/build/loki-text-{hash}/out)
    // up to target/{profile}/.
    // OUT_DIR depth from profile dir: out → loki-text-{hash} → build → {profile}
    let profile_dir = out_dir
        .ancestors()
        .nth(3)
        .expect("unexpected OUT_DIR structure")
        .to_path_buf();

    let src = manifest_dir.join("assets");
    let dst = profile_dir.join("assets");

    if src.exists() && !dst.exists() {
        #[cfg(unix)]
        {
            if let Err(e) = std::os::unix::fs::symlink(&src, &dst) {
                // Non-fatal: symlink might fail in some environments (e.g. read-only FS).
                // The app will fall back to system fonts if the asset isn't resolved.
                eprintln!(
                    "cargo:warning=build.rs: could not create assets symlink \
                     {} -> {}: {}",
                    dst.display(),
                    src.display(),
                    e
                );
            }
        }
        #[cfg(not(unix))]
        {
            // On Windows, fall back to recursively copying the assets directory
            // since symlinks require elevated permissions.
            if let Err(e) = copy_dir_all(&src, &dst) {
                eprintln!(
                    "cargo:warning=build.rs: could not copy assets {} -> {}: {}",
                    src.display(),
                    dst.display(),
                    e
                );
            }
        }
    }

    // Re-run this build script if any asset changes.
    println!("cargo:rerun-if-changed=assets/");
}

#[cfg(not(unix))]
fn copy_dir_all(src: &std::path::Path, dst: &std::path::Path) -> std::io::Result<()> {
    fs::create_dir_all(dst)?;
    for entry in fs::read_dir(src)? {
        let entry = entry?;
        let ty = entry.file_type()?;
        if ty.is_dir() {
            copy_dir_all(&entry.path(), &dst.join(entry.file_name()))?;
        } else {
            fs::copy(entry.path(), dst.join(entry.file_name()))?;
        }
    }
    Ok(())
}
