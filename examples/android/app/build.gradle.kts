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

    buildTypes {
        release {
            isMinifyEnabled = false
            proguardFiles(
                getDefaultProguardFile("proguard-android-optimize.txt"),
                "proguard-rules.pro"
            )
            // Sign release with debug keystore for local installs
            signingConfig = signingConfigs.getByName("debug")
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
