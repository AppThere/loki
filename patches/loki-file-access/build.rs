// SPDX-License-Identifier: MIT
// Copyright (c) 2026 AppThere

//! Build script — compiles `FilePickerActivity.java` into `classes.dex` for Android targets.
//!
//! Requires:
//! - `ANDROID_HOME` or `ANDROID_SDK_ROOT` pointing to the Android SDK
//! - `javac` on PATH, in `JAVA_HOME/bin`, or in Android Studio's bundled JDK
//!
//! On non-Android targets this script does nothing.

use std::path::PathBuf;

fn main() {
    println!("cargo:rerun-if-changed=android/FilePickerActivity.java");

    let target_os = std::env::var("CARGO_CFG_TARGET_OS").unwrap_or_default();
    if target_os != "android" {
        return;
    }

    let manifest_dir = PathBuf::from(std::env::var("CARGO_MANIFEST_DIR").unwrap());
    let out_dir = PathBuf::from(std::env::var("OUT_DIR").unwrap());
    let java_src = manifest_dir.join("android/FilePickerActivity.java");
    let dex_out = out_dir.join("classes.dex");

    match compile_to_dex(&java_src, &out_dir, &dex_out) {
        Ok(()) => {
            println!("cargo:warning=FilePickerActivity DEX: {}", dex_out.display());
            println!("cargo:warning=Inject into APK with: scripts/build-android.ps1 -Install");
        }
        Err(e) => {
            println!("cargo:warning=FilePickerActivity compile skipped: {e}");
            println!("cargo:warning=Run scripts/build-android.ps1 which compiles the DEX itself.");
        }
    }

    println!("cargo:rustc-env=LOKI_FILE_ACCESS_DEX={}", dex_out.display());
}

fn compile_to_dex(
    java_src: &std::path::Path,
    out_dir: &std::path::Path,
    dex_out: &std::path::Path,
) -> Result<(), String> {
    let android_home = std::env::var("ANDROID_HOME")
        .or_else(|_| std::env::var("ANDROID_SDK_ROOT"))
        .map_err(|_| "ANDROID_HOME not set".to_owned())?;
    let android_home = PathBuf::from(android_home);

    let android_jar = find_android_jar(&android_home)?;
    let d8 = find_d8(&android_home)?;
    let javac = find_javac();

    let classes_dir = out_dir.join("java_classes");
    std::fs::create_dir_all(&classes_dir).map_err(|e| format!("mkdir classes: {e}"))?;

    let status = std::process::Command::new(&javac)
        .args([
            "-source", "8",
            "-target", "8",
            "-classpath",
            android_jar.to_str().unwrap(),
            "-d",
            classes_dir.to_str().unwrap(),
            java_src.to_str().unwrap(),
        ])
        .status()
        .map_err(|e| format!("javac exec failed: {e}"))?;
    if !status.success() {
        return Err(format!("javac exited {status}"));
    }

    let class_file = classes_dir
        .join("io/github/appthere/lokifileaccess/FilePickerActivity.class");

    let dex_dir = out_dir.join("dex_out");
    std::fs::create_dir_all(&dex_dir).map_err(|e| format!("mkdir dex: {e}"))?;

    let status = std::process::Command::new(&d8)
        .args([
            class_file.to_str().unwrap(),
            "--output",
            dex_dir.to_str().unwrap(),
            "--min-api",
            "26",
        ])
        .status()
        .map_err(|e| format!("d8 exec failed: {e}"))?;
    if !status.success() {
        return Err(format!("d8 exited {status}"));
    }

    std::fs::copy(dex_dir.join("classes.dex"), dex_out)
        .map_err(|e| format!("copy dex: {e}"))?;

    Ok(())
}

fn find_android_jar(android_home: &std::path::Path) -> Result<PathBuf, String> {
    let platforms = android_home.join("platforms");
    for api in (26..=36).rev() {
        let jar = platforms.join(format!("android-{api}")).join("android.jar");
        if jar.exists() {
            return Ok(jar);
        }
    }
    Err("android.jar not found under $ANDROID_HOME/platforms/android-*/".to_owned())
}

fn find_d8(android_home: &std::path::Path) -> Result<PathBuf, String> {
    let build_tools = android_home.join("build-tools");
    let mut entries: Vec<_> = std::fs::read_dir(&build_tools)
        .map_err(|e| format!("read build-tools: {e}"))?
        .filter_map(|e| e.ok())
        .collect();
    entries.sort_by(|a, b| b.file_name().cmp(&a.file_name()));
    for entry in entries {
        for name in &["d8.bat", "d8"] {
            let d8 = entry.path().join(name);
            if d8.exists() {
                return Ok(d8);
            }
        }
    }
    Err("d8 not found under $ANDROID_HOME/build-tools/".to_owned())
}

fn find_javac() -> PathBuf {
    if let Ok(java_home) = std::env::var("JAVA_HOME") {
        for name in &["bin/javac", "bin/javac.exe"] {
            let p = PathBuf::from(&java_home).join(name);
            if p.exists() {
                return p;
            }
        }
    }
    // Android Studio bundled JDK (Windows — Program Files location)
    #[cfg(target_os = "windows")]
    {
        let pf = std::env::var("PROGRAMFILES").unwrap_or_else(|_| "C:\\Program Files".into());
        let p = PathBuf::from(pf)
            .join("Android\\Android Studio\\jbr\\bin\\javac.exe");
        if p.exists() {
            return p;
        }
    }
    PathBuf::from(if cfg!(target_os = "windows") { "javac.exe" } else { "javac" })
}
