// SPDX-License-Identifier: Apache-2.0

//! Build script for loki-presentation.
//!
//! Creates a symlink or copies `assets/` to target build profile directory.

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
