<#
.SYNOPSIS
    Full Android build pipeline for Loki apps including optional Java shims.

.DESCRIPTION
    1. Compiles the Java shims (FilePickerActivity, ImeInsetsListener) → classes.dex (loki-text only).
    2. Runs `cargo apk build` to produce the native library + bare APK.
    3. Post-processes the APK:
         a. Replaces the auto-generated AndroidManifest.xml with the custom one
            from <app>/AndroidManifest.xml (loki-text also adds FilePickerActivity
            + hasCode=true; other apps use the simpler NativeActivity-only manifest).
         b. Injects classes.dex (loki-text only).
    4. Zipaligns and signs with the debug keystore.
    5. Optionally installs via adb.

.PARAMETER App
    Which app to build: text (default), spreadsheet, or presentation.

.PARAMETER Release
    Build a release APK instead of debug.

.PARAMETER Install
    Install the finished APK on a connected device/emulator via adb.

.PARAMETER SkipCargoApk
    Skip cargo apk build (useful when only the manifest/DEX changed).

.PARAMETER Gpu
    Enable the real Vello GPU renderer (VelloWindowRenderer / use_wgpu).
    Requires a Vulkan-capable physical device; omit for the Android emulator
    (which uses SwiftShader and lacks the compute shader support Vello needs).

.EXAMPLE
    .\scripts\build-android.ps1 -Install
    .\scripts\build-android.ps1 -Release -Install -Gpu
    .\scripts\build-android.ps1 -App spreadsheet -Release -Install -Gpu
    .\scripts\build-android.ps1 -App presentation -Release -Install -Gpu
#>

param(
    [ValidateSet("text", "spreadsheet", "presentation")]
    [string]$App = "text",
    [switch]$Release,
    [switch]$Install,
    [switch]$SkipCargoApk,
    # Pass -Gpu to enable the real Vello GPU renderer (VelloWindowRenderer / use_wgpu).
    # Requires a Vulkan-capable physical device; omit for the Android emulator
    # (which uses SwiftShader and lacks the compute shader support Vello needs).
    [switch]$Gpu,
    # Which ABI(s) to build:
    #   auto  (default) On -Install, detect the connected device's ABI and build
    #         only that target; otherwise build all ABIs (universal APK).
    #   arm64 Build only aarch64-linux-android (arm64-v8a).
    #   x64   Build only x86_64-linux-android   (x86_64; Chromebooks/ARC).
    #   all   Universal multi-ABI APK (both targets from Cargo.toml build_targets).
    [ValidateSet("auto", "arm64", "x64", "all")]
    [string]$Abi = "auto"
)

$ErrorActionPreference = "Stop"
Set-Location (Split-Path $PSScriptRoot -Parent)

# Ensure Android NDK is set for cargo-apk.
if (-not $env:ANDROID_NDK_ROOT) {
    $ndkBase = "$env:LOCALAPPDATA\Android\Sdk\ndk"
    if (Test-Path $ndkBase) {
        $env:ANDROID_NDK_ROOT = (Get-ChildItem $ndkBase | Sort-Object Name -Descending | Select-Object -First 1 -ExpandProperty FullName)
        Write-Host "==> Auto-detected NDK: $env:ANDROID_NDK_ROOT"
    } else {
        throw "ANDROID_NDK_ROOT not set and $ndkBase not found."
    }
}

# ── Tool paths ────────────────────────────────────────────────────────────────

$sdk        = $env:LOCALAPPDATA + "\Android\Sdk"
$btVer      = (Get-ChildItem "$sdk\build-tools" | Sort-Object Name -Descending | Select-Object -First 1).Name
$bt         = "$sdk\build-tools\$btVer"
$aapt       = "$bt\aapt.exe"
$d8         = "$bt\d8.bat"
$zipalign   = "$bt\zipalign.exe"
$apksigner  = "$bt\apksigner.bat"
$platform   = (Get-ChildItem "$sdk\platforms\android-*" | Sort-Object Name -Descending | Select-Object -First 1).FullName + "\android.jar"
# Prefer JAVA_HOME, then Android Studio bundled JDK, then system PATH.
if ($env:JAVA_HOME -and (Test-Path "$env:JAVA_HOME\bin\javac.exe")) {
    $javac = "$env:JAVA_HOME\bin\javac.exe"
} elseif (Test-Path "C:\Program Files\Android\Android Studio\jbr\bin\javac.exe") {
    $javac = "C:\Program Files\Android\Android Studio\jbr\bin\javac.exe"
} else {
    $javacCmd = Get-Command javac -ErrorAction SilentlyContinue
    $javac = if ($javacCmd) { $javacCmd.Source } else { $null }
    if (-not $javac) { throw "javac not found. Set JAVA_HOME, install JDK, or install Android Studio." }
}
$debugKey   = "$env:USERPROFILE\.android\debug.keystore"

Write-Host "==> Build tools: $bt"
Write-Host "==> Platform:    $platform"
Write-Host "==> javac:       $javac"

# ── App-specific config ───────────────────────────────────────────────────────

switch ($App) {
    "text" {
        $cargoPackage  = "loki-text"
        $apkBaseName   = "loki_text"
        $manifestXml   = "loki-text\AndroidManifest.xml"
        $includeDex    = $true
        $launchActivity = "com.appthere.loki/android.app.NativeActivity"
    }
    "spreadsheet" {
        $cargoPackage  = "loki-spreadsheet"
        $apkBaseName   = "loki_spreadsheet"
        $manifestXml   = "loki-spreadsheet\AndroidManifest.xml"
        $includeDex    = $false
        $launchActivity = "com.appthere.loki.spreadsheet/android.app.NativeActivity"
    }
    "presentation" {
        $cargoPackage  = "loki-presentation"
        $apkBaseName   = "loki_presentation"
        $manifestXml   = "loki-presentation\AndroidManifest.xml"
        $includeDex    = $false
        $launchActivity = "com.appthere.loki.presentation/android.app.NativeActivity"
    }
}

Write-Host "==> App:         $App ($cargoPackage)"

# ── Paths ─────────────────────────────────────────────────────────────────────

$profile       = if ($Release) { "release" } else { "debug" }
$javaSrcs      = @(
    "patches\loki-file-access\android\FilePickerActivity.java",
    "patches\loki-file-access\android\ImeInsetsListener.java"
)
$outDir        = "target\android-pkg"
$apkSrc        = "$PWD\target\$profile\apk\$apkBaseName.apk"

New-Item -ItemType Directory -Force $outDir | Out-Null
$outDir = (Resolve-Path $outDir).Path   # make absolute for aapt

# ── Step 0: Stage debug keystore for cargo-apk release signing ───────────────
# cargo-apk refuses to build a release APK without a signing config.
# We satisfy it by copying the Android Studio debug keystore to a path inside
# target/android-pkg/ before cargo apk runs.  Our own apksigner step (Step 6)
# re-signs the final APK with the same key, so cargo-apk's signature is
# overwritten and serves only to unblock the build.
$stagedKeystore = "$outDir\signing.keystore"
if (Test-Path $debugKey) {
    Copy-Item $debugKey $stagedKeystore -Force
    Write-Host "==> Staged signing keystore: $stagedKeystore"
} else {
    throw "Android debug keystore not found at $debugKey.`nRun Android Studio once to generate it, or set a different path."
}

# ── Step 1: Compile FilePickerActivity.java → classes.dex (loki-text only) ───

if ($includeDex) {
    Write-Host "`n==> Compiling Java shims (FilePickerActivity, ImeInsetsListener)..."
    $classesDir = "$outDir\java-classes"
    $dexDir     = "$outDir\dex-out"
    New-Item -ItemType Directory -Force $classesDir, $dexDir | Out-Null

    & $javac -source 8 -target 8 -classpath $platform -d $classesDir @javaSrcs
    if ($LASTEXITCODE -ne 0) { throw "javac failed" }

    # Dex every produced .class (includes inner classes such as the anonymous
    # Runnable in ImeInsetsListener).
    $classFiles = Get-ChildItem -Path $classesDir -Recurse -Filter *.class | ForEach-Object { $_.FullName }
    if ($classFiles.Count -eq 0) { throw "javac produced no .class files" }
    & $d8 @classFiles --output $dexDir --min-api 26
    if ($LASTEXITCODE -ne 0) { throw "d8 failed" }

    $dexPath = "$dexDir\classes.dex"
    Write-Host "    DEX: $dexPath"
}

# ── Step 2: cargo apk build ───────────────────────────────────────────────────

if (-not $SkipCargoApk) {
    Write-Host "`n==> cargo apk build ($profile, app=$App)..."
    # Resolve the cargo --target from -Abi.  Empty => build all ABIs from
    # Cargo.toml build_targets (universal APK); non-empty => single --target.
    $cargoTarget = ""
    switch ($Abi) {
        "arm64" { $cargoTarget = "aarch64-linux-android" }
        "x64"   { $cargoTarget = "x86_64-linux-android" }
        "all"   { $cargoTarget = "" }
        "auto"  {
            if ($Install) {
                $devAbi = (& adb shell getprop ro.product.cpu.abi).Trim()
                switch -Wildcard ($devAbi) {
                    "arm64-v8a" { $cargoTarget = "aarch64-linux-android" }
                    "x86_64"    { $cargoTarget = "x86_64-linux-android" }
                    "armeabi*"  { $cargoTarget = "armv7-linux-androideabi" }
                    "x86"       { $cargoTarget = "i686-linux-android" }
                }
                if ($cargoTarget) { Write-Host "    Auto-detected device ABI -> $cargoTarget" }
                else { Write-Host "    No device ABI detected; building all ABIs (universal APK)" }
            }
        }
    }
    $buildArgs = @("apk", "build", "--package", $cargoPackage)
    if ($Release) { $buildArgs += "--release" }
    if ($cargoTarget) { $buildArgs += @("--target", $cargoTarget) }
    # On a physical Vulkan device, -Gpu enables the full Vello GPU renderer.
    # The android_gpu cfg flag is checked throughout dioxus-native and loki-renderer.
    if ($Gpu -and ($env:RUSTFLAGS -notlike "*--cfg android_gpu*")) {
        $env:RUSTFLAGS = ($env:RUSTFLAGS + " --cfg android_gpu").Trim()
    }
    if ($Gpu) {
        Write-Host "    GPU renderer enabled (RUSTFLAGS: $env:RUSTFLAGS)"
    }
    & cargo @buildArgs
    # cargo-apk may exit non-zero due to a post-build artifact-check panic in
    # cargo-subcommand (Bin vs Cdylib confusion) even when the APK was built
    # successfully.  Check for the APK directly instead of trusting exit code.
    if ($LASTEXITCODE -ne 0 -and -not (Test-Path $apkSrc)) {
        throw "cargo apk build failed and APK not found at $apkSrc"
    }
}

if (-not (Test-Path $apkSrc)) {
    throw "APK not found at $apkSrc - run without -SkipCargoApk first."
}

# ── Step 3: Generate binary AndroidManifest.xml via aapt ─────────────────────

Write-Host "`n==> Packaging custom AndroidManifest.xml → binary AXML..."
$manifestApk     = "$outDir\manifest-only-$App.apk"
$manifestExtract = "$outDir\manifest-extract-$App"
# Always start with an empty extraction directory so that re-runs do not fail
# on "file already exists" from the previous build's AndroidManifest.xml.
if (Test-Path $manifestExtract) { Remove-Item -Recurse -Force $manifestExtract }
New-Item -ItemType Directory -Force $manifestExtract | Out-Null

& $aapt package -f -F $manifestApk -M $manifestXml -I $platform
if ($LASTEXITCODE -ne 0) { throw "aapt package (manifest) failed" }

# Extract the binary AXML manifest from the temporary APK.
Add-Type -Assembly System.IO.Compression.FileSystem
[System.IO.Compression.ZipFile]::ExtractToDirectory($manifestApk, $manifestExtract)
if (-not (Test-Path "$manifestExtract\AndroidManifest.xml")) {
    throw "Binary manifest not found in aapt output"
}

# ── Step 4: Patch the cargo-apk APK ──────────────────────────────────────────

Write-Host "`n==> Patching APK (replace manifest$(if ($includeDex) {' + inject DEX'}))..."
$apkWork = "$outDir\$apkBaseName-patched.apk"
Copy-Item $apkSrc $apkWork -Force

# Replace binary AndroidManifest.xml
Push-Location $manifestExtract
& $aapt remove $apkWork AndroidManifest.xml 2>$null
& $aapt add $apkWork AndroidManifest.xml
if ($LASTEXITCODE -ne 0) { throw "aapt add manifest failed" }
Pop-Location

# Inject classes.dex (loki-text only — needs FilePickerActivity)
if ($includeDex) {
    Push-Location $dexDir
    & $aapt add $apkWork classes.dex
    if ($LASTEXITCODE -ne 0) { throw "aapt add dex failed" }
    Pop-Location
}

# ── Step 5: Zipalign ─────────────────────────────────────────────────────────

Write-Host "`n==> Zipaligning..."
$apkAligned = "$outDir\$apkBaseName-aligned.apk"
Remove-Item $apkAligned -ErrorAction SilentlyContinue
& $zipalign -f 4 $apkWork $apkAligned
if ($LASTEXITCODE -ne 0) { throw "zipalign failed" }

# ── Step 6: Sign ─────────────────────────────────────────────────────────────

Write-Host "`n==> Signing with debug keystore..."
& $apksigner sign `
    --ks $debugKey `
    --ks-key-alias androiddebugkey `
    --ks-pass pass:android `
    --key-pass pass:android `
    $apkAligned
if ($LASTEXITCODE -ne 0) { throw "apksigner failed" }

Write-Host "`n==> APK ready: $apkAligned"

# ── Step 7: Install ───────────────────────────────────────────────────────────

if ($Install) {
    Write-Host "`n==> Installing $App on device..."
    # -r: replace existing; -d: allow version downgrade (dev builds use version code 0).
    & adb install -r -d $apkAligned
    if ($LASTEXITCODE -ne 0) { throw "adb install failed" }
    Write-Host "==> Installed successfully!"
    Write-Host "==> Launch: adb shell am start -n $launchActivity"
}
