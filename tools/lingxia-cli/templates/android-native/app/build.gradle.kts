plugins {
    alias(libs.plugins.android.application)
    alias(libs.plugins.kotlin.android)
}

val lingxiaApplicationIdSuffix = providers
    .gradleProperty("lingxia.applicationIdSuffix")
    .orElse("")
    .get()
val lingxiaAppName = providers
    .gradleProperty("lingxia.appName")
    .orElse("{{PRODUCT_NAME}}")
    .get()
// Optional res overlay dir injected by `lingxia build` for env-specific
// resources (e.g. dev/preview launcher-icon badges). The CLI generates files
// outside the source tree and points us at them so AGP merges them with the
// project's own resources. No source-tree mutation.
val lingxiaResOverlayDir = providers
    .gradleProperty("lingxia.resOverlayDir")
    .orNull
// Manifest icon refs. When `lingxia build` injects an env-specific overlay
// it also passes these so the manifest's `${lxAppIcon}` / `${lxAppRoundIcon}`
// placeholders point at the overlay-only drawables (avoiding duplicate-
// resource errors that come from overriding `ic_launcher.xml` in place).
val lingxiaAppIcon = providers
    .gradleProperty("lingxia.appIcon")
    .orElse("@mipmap/ic_launcher")
    .get()
val lingxiaAppRoundIcon = providers
    .gradleProperty("lingxia.appRoundIcon")
    .orElse("@mipmap/ic_launcher_round")
    .get()

android {
    namespace = "{{PACKAGE_ID}}"
    compileSdk = {{COMPILE_SDK}}

    defaultConfig {
        applicationId = "{{PACKAGE_ID}}"
        if (lingxiaApplicationIdSuffix.isNotEmpty()) {
            applicationIdSuffix = lingxiaApplicationIdSuffix
        }
        manifestPlaceholders["lxAppName"] = lingxiaAppName
        manifestPlaceholders["lxAppIcon"] = lingxiaAppIcon
        manifestPlaceholders["lxAppRoundIcon"] = lingxiaAppRoundIcon
        minSdk = {{MIN_SDK}}
        targetSdk = {{TARGET_SDK}}
        versionCode = 1
        versionName = "1.0"
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
    // LingXia SDK. The Maven repo is injected by `lingxia build` (see
    // settings.gradle.kts). `lingxia.sdkVersion` is passed by the CLI to keep
    // the coordinate aligned with the fetched artifact; the baked
    // `{{SDK_VERSION}}` is the fallback for direct Gradle invocations.
    val lingxiaSdkVersion = (findProperty("lingxia.sdkVersion") as String?) ?: "{{SDK_VERSION}}"
    implementation("io.github.lingxia-dev:lingxia:$lingxiaSdkVersion")

    // Android dependencies
    implementation(libs.androidx.core.ktx)
    implementation(libs.androidx.appcompat)
    implementation(libs.material)
}
