plugins {
    id("com.android.application")
    id("org.jetbrains.kotlin.plugin.compose")
    id("org.jetbrains.kotlin.plugin.serialization")
}

// One version for every Valo build: the workspace's Cargo.toml.
val appVersion: String = rootProject.file("../../Cargo.toml").readLines()
    .first { it.startsWith("version") }
    .substringAfter('"').substringBefore('"')

// Release builds are signed with the keystore these point at (CI keeps it in
// secrets). Without one they fall back to the debug key, which can't update
// an install signed with a different key.
val keystore: String? = System.getenv("VALO_KEYSTORE")

android {
    namespace = "io.github.okaza03.valo"
    compileSdk = 37

    defaultConfig {
        applicationId = "io.github.okaza03.valo"
        minSdk = 24
        targetSdk = 35
        versionName = appVersion
        // 0.4.0 -> 400, 1.2.3 -> 10203
        versionCode = appVersion.split('.').map { it.toInt() }.fold(0) { acc, n -> acc * 100 + n }
        // where "check for updates" looks; -Pvalo.releases=<url> points a test build elsewhere
        val releases = (project.findProperty("valo.releases") as String?)
            ?: "https://api.github.com/repos/Okaza03/Valo/releases/latest"
        buildConfigField("String", "RELEASES_URL", "\"$releases\"")
    }

    // One codebase, two apps: src/main is shared, src/tv and src/phone have the
    // screens. They install side by side, and each updates from its own APK.
    flavorDimensions += "device"
    productFlavors {
        create("tv") {
            dimension = "device"
            buildConfigField("String", "APK_SUFFIX", "\"-androidtv.apk\"")
            buildConfigField("String", "DEVICE_NAME", "\"Valo TV\"")
            buildConfigField("String", "USER_AGENT", "\"Valo-AndroidTV\"")
        }
        create("phone") {
            dimension = "device"
            applicationIdSuffix = ".phone"
            buildConfigField("String", "APK_SUFFIX", "\"-android.apk\"")
            buildConfigField("String", "DEVICE_NAME", "\"Valo\"")
            buildConfigField("String", "USER_AGENT", "\"Valo-Android\"")
        }
    }

    signingConfigs {
        if (keystore != null) {
            create("release") {
                storeFile = file(keystore)
                storePassword = System.getenv("VALO_KEYSTORE_PASSWORD")
                keyAlias = System.getenv("VALO_KEY_ALIAS") ?: "valo"
                keyPassword = System.getenv("VALO_KEY_PASSWORD")
                    ?: System.getenv("VALO_KEYSTORE_PASSWORD")
            }
        }
    }

    buildTypes {
        release {
            isMinifyEnabled = true
            isShrinkResources = true
            proguardFiles(getDefaultProguardFile("proguard-android-optimize.txt"), "proguard-rules.pro")
            signingConfig = signingConfigs.findByName("release") ?: signingConfigs.getByName("debug")
        }
    }

    compileOptions {
        sourceCompatibility = JavaVersion.VERSION_17
        targetCompatibility = JavaVersion.VERSION_17
    }
    buildFeatures {
        compose = true
        buildConfig = true
    }
}

dependencies {
    val media3 = "1.11.1"
    val composeBom = platform("androidx.compose:compose-bom:2026.09.00")
    implementation(composeBom)
    implementation("androidx.compose.foundation:foundation")
    implementation("androidx.compose.ui:ui")
    "tvImplementation"("androidx.tv:tv-material:1.1.0")
    "phoneImplementation"("androidx.compose.material3:material3")
    // the Cast button, and playing on a Chromecast or Google TV
    "phoneImplementation"("com.google.android.gms:play-services-cast-framework:23.0.0")
    "phoneImplementation"("androidx.mediarouter:mediarouter:1.8.1")
    "phoneImplementation"("androidx.appcompat:appcompat:1.8.0")
    // surround sound as stereo, for Chromecasts that won't take it (StereoStream.kt)
    "phoneImplementation"("androidx.media3:media3-muxer:$media3")
    implementation("androidx.activity:activity-compose:1.13.0")
    implementation("androidx.lifecycle:lifecycle-viewmodel-compose:2.11.0")
    implementation("androidx.lifecycle:lifecycle-runtime-compose:2.11.0")
    implementation("androidx.core:core-ktx:1.19.1")

    implementation("androidx.media3:media3-exoplayer:$media3")
    // subtitle files are parsed and drawn by the app, so their timing can move
    implementation("androidx.media3:media3-extractor:$media3")
    implementation("androidx.media3:media3-datasource-okhttp:$media3")
    implementation("androidx.media3:media3-ui:$media3")

    implementation("com.squareup.okhttp3:okhttp:5.5.1")
    implementation("io.coil-kt.coil3:coil-compose:3.6.3")
    implementation("io.coil-kt.coil3:coil-network-okhttp:3.6.3")
    implementation("org.jetbrains.kotlinx:kotlinx-serialization-json:1.12.0")
    implementation("org.jetbrains.kotlinx:kotlinx-coroutines-android:1.11.0")

    testImplementation("junit:junit:4.13.2")
}

kotlin {
    compilerOptions {
        jvmTarget.set(org.jetbrains.kotlin.gradle.dsl.JvmTarget.JVM_17)
    }
}
