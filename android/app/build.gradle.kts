// SPDX-License-Identifier: Apache-2.0
plugins {
    // Pure-native app: the only Java is FilePickerActivity, so no Kotlin plugin.
    id("com.android.application")
}

android {
    namespace = "com.appthere.loki"
    compileSdk = 34

    defaultConfig {
        applicationId = "com.appthere.loki"
        // Vulkan (and thus the Blitz/wgpu renderer) requires API 26+.
        minSdk = 26
        targetSdk = 34
        // Overridable from CI so each Play upload gets a fresh, increasing code.
        versionCode = (System.getenv("LOKI_VERSION_CODE") ?: "1").toInt()
        versionName = System.getenv("LOKI_VERSION_NAME") ?: "0.1.0"
        // NOTE: ABIs are NOT filtered here — the set of architectures shipped is
        // whatever scripts/build-aab.sh stages into src/main/jniLibs/<abi>/.
        // That keeps "build for one ABI" vs "build universal" a script concern.
    }

    signingConfigs {
        // Defaults to the Android debug keystore so a plain build yields an
        // installable (bundletool-testable) AAB out of the box.  For a real Play
        // upload, point these env vars at your upload keystore.
        create("upload") {
            val home = System.getProperty("user.home")
            storeFile = file(System.getenv("LOKI_KEYSTORE") ?: "$home/.android/debug.keystore")
            storePassword = System.getenv("LOKI_KEYSTORE_PASS") ?: "android"
            keyAlias = System.getenv("LOKI_KEY_ALIAS") ?: "androiddebugkey"
            keyPassword = System.getenv("LOKI_KEY_PASS") ?: "android"
        }
    }

    buildTypes {
        getByName("release") {
            // FilePickerActivity is resolved by fully-qualified name over JNI from
            // Rust (loki-file-access).  R8/ProGuard would rename or strip it and
            // break file open/save, so minification stays OFF.
            isMinifyEnabled = false
            signingConfig = signingConfigs.getByName("upload")
        }
    }

    compileOptions {
        sourceCompatibility = JavaVersion.VERSION_17
        targetCompatibility = JavaVersion.VERSION_17
    }

    // targetSdk 34 can trip the ExpiredTargetSdkVersion lint to an error in
    // later years; do not let lint fail the bundle build.
    lint {
        abortOnError = false
        checkReleaseBuilds = false
    }

    sourceSets {
        getByName("main") {
            java.srcDirs("src/main/java")
            jniLibs.srcDirs("src/main/jniLibs")
        }
    }
}
