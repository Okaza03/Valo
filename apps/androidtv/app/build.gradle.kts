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
    val composeBom = platform("androidx.compose:compose-bom:2026.09.00")
    implementation(composeBom)
    implementation("androidx.compose.foundation:foundation")
    implementation("androidx.compose.ui:ui")
    implementation("androidx.tv:tv-material:1.1.0")
    implementation("androidx.activity:activity-compose:1.9.3")
    implementation("androidx.lifecycle:lifecycle-viewmodel-compose:2.8.7")
    implementation("androidx.lifecycle:lifecycle-runtime-compose:2.8.7")
    implementation("androidx.core:core-ktx:1.19.1")

    val media3 = "1.11.1"
    implementation("androidx.media3:media3-exoplayer:$media3")
    // subtitle files are parsed and drawn by the app, so their timing can move
    implementation("androidx.media3:media3-extractor:$media3")
    implementation("androidx.media3:media3-datasource-okhttp:$media3")
    implementation("androidx.media3:media3-ui:$media3")

    implementation("com.squareup.okhttp3:okhttp:5.5.0")
    implementation("io.coil-kt.coil3:coil-compose:3.6.3")
    implementation("io.coil-kt.coil3:coil-network-okhttp:3.6.3")
    implementation("org.jetbrains.kotlinx:kotlinx-serialization-json:1.11.0")
    implementation("org.jetbrains.kotlinx:kotlinx-coroutines-android:1.11.0")

    testImplementation("junit:junit:4.13.2")
}

kotlin {
    compilerOptions {
        jvmTarget.set(org.jetbrains.kotlin.gradle.dsl.JvmTarget.JVM_17)
    }
}
