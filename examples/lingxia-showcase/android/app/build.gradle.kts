import java.util.Properties

plugins {
    alias(libs.plugins.android.application)
    alias(libs.plugins.kotlin.android)
}

val requestedMinSdk = (project.findProperty("MIN_SDK") as String?)?.toIntOrNull() ?: 29
val lingxiaApplicationIdSuffix = providers
    .gradleProperty("lingxia.applicationIdSuffix")
    .orElse("")
    .get()
val lingxiaAppName = providers
    .gradleProperty("lingxia.appName")
    .orElse("LingXia App Demo")
    .get()
val lingxiaResOverlayDir = providers
    .gradleProperty("lingxia.resOverlayDir")
    .orNull
val lingxiaAppIcon = providers
    .gradleProperty("lingxia.appIcon")
    .orElse("@mipmap/ic_launcher")
    .get()
val lingxiaAppRoundIcon = providers
    .gradleProperty("lingxia.appRoundIcon")
    .orElse("@mipmap/ic_launcher_round")
    .get()

// Release signing — values come from keystore.properties (local) or matching
// env vars (CI). When none are set the build falls back to the debug keystore
// so it still produces an installable APK (local dev / a fork without secrets).
val keystorePropertiesFile = rootProject.file("keystore.properties")
val keystoreProperties = Properties()
if (keystorePropertiesFile.exists()) {
    keystorePropertiesFile.inputStream().use { stream -> keystoreProperties.load(stream) }
}
fun getSigningValue(name: String): String? {
    val fileValue = keystoreProperties.getProperty(name)?.trim()
    if (!fileValue.isNullOrEmpty()) {
        return fileValue
    }
    val envValue = System.getenv(name)?.trim()
    if (!envValue.isNullOrEmpty()) {
        return envValue
    }
    return null
}
val releaseStoreFile = getSigningValue("RELEASE_STORE_FILE")
val releaseStorePassword = getSigningValue("RELEASE_STORE_PASSWORD")
val releaseKeyAlias = getSigningValue("RELEASE_KEY_ALIAS")
val releaseKeyPassword = getSigningValue("RELEASE_KEY_PASSWORD")
val releaseSigningEnabled = releaseStoreFile != null &&
    releaseStorePassword != null &&
    releaseKeyAlias != null &&
    releaseKeyPassword != null

// Loudly flag a release build that falls back to the debug keystore (no release
// keystore configured): it installs for testing but cannot be distributed.
if (!releaseSigningEnabled) {
    gradle.taskGraph.whenReady {
        val buildingRelease = allTasks.any {
            it.name.endsWith("Release") &&
                (it.name.startsWith("assemble") ||
                    it.name.startsWith("bundle") ||
                    it.name.startsWith("package"))
        }
        if (buildingRelease) {
            logger.warn(
                "No release keystore configured: the release build is DEBUG-signed " +
                    "and NOT distributable. Set keystore.properties or the RELEASE_* " +
                    "env vars to sign for release."
            )
        }
    }
}

android {
    namespace = "com.lingxia.example.lxapp"
    compileSdk = 35

    defaultConfig {
        applicationId = "com.lingxia.example.lxapp"
        if (lingxiaApplicationIdSuffix.isNotEmpty()) {
            applicationIdSuffix = lingxiaApplicationIdSuffix
        }
        manifestPlaceholders["lxAppName"] = lingxiaAppName
        manifestPlaceholders["lxAppIcon"] = lingxiaAppIcon
        manifestPlaceholders["lxAppRoundIcon"] = lingxiaAppRoundIcon
        minSdk = requestedMinSdk
        targetSdk = 35
        versionCode = 1
        versionName = "1.0"

        testInstrumentationRunner = "androidx.test.runner.AndroidJUnitRunner"
    }

    sourceSets.getByName("main") {
        lingxiaResOverlayDir?.let { dir ->
            res.srcDir(file(dir))
        }
    }

    signingConfigs {
        if (releaseSigningEnabled) {
            create("release") {
                storeFile = rootProject.file(requireNotNull(releaseStoreFile))
                storePassword = requireNotNull(releaseStorePassword)
                keyAlias = requireNotNull(releaseKeyAlias)
                keyPassword = requireNotNull(releaseKeyPassword)
            }
        }
    }

    buildTypes {
        release {
            isMinifyEnabled = false
            proguardFiles(
                getDefaultProguardFile("proguard-android-optimize.txt"),
                "proguard-rules.pro"
            )
            // Real release keystore when configured (keystore.properties / env);
            // otherwise debug-sign so the build still yields an installable APK.
            signingConfig = if (releaseSigningEnabled) {
                signingConfigs.getByName("release")
            } else {
                signingConfigs.getByName("debug")
            }
        }
    }
    compileOptions {
        sourceCompatibility = JavaVersion.VERSION_11
        targetCompatibility = JavaVersion.VERSION_11
    }
    kotlinOptions {
        jvmTarget = "11"
    }
}

dependencies {
    implementation(project(":lingxia"))

    // App's own dependencies
    implementation(libs.androidx.core.ktx)
    implementation(libs.androidx.appcompat)
    implementation(libs.material)
}
