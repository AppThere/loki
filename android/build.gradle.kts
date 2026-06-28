// SPDX-License-Identifier: Apache-2.0
buildscript {
    repositories {
        google()
        mavenCentral()
    }
    dependencies {
        // Versions match the toolchain dx 0.7.9 generates and that is known to
        // build successfully in this environment (Gradle 9.1.0 wrapper).
        classpath("com.android.tools.build:gradle:8.7.0")
    }
}

allprojects {
    repositories {
        google()
        mavenCentral()
    }
}

tasks.register("clean") {
    delete(rootProject.layout.buildDirectory)
}
