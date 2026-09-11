import com.vanniktech.maven.publish.AndroidSingleVariantLibrary

plugins {
    id("com.android.library")
    id("org.jetbrains.kotlin.android")
    id("maven-publish")
    id("com.vanniktech.maven.publish") version "0.34.0" apply false
}

val targetSdkProp = (project.findProperty("targetSdk") as String?)?.toIntOrNull() ?: 35
val compileSdkProp = (project.findProperty("compileSdk") as String?)?.toIntOrNull() ?: 35

android {
    namespace = "com.lingxia.lxapp"
    compileSdk = compileSdkProp

    defaultConfig {
        minSdk = 21
        lint.targetSdk = targetSdkProp

        testInstrumentationRunner = "androidx.test.runner.AndroidJUnitRunner"
        consumerProguardFiles("consumer-rules.pro")
    }

    buildTypes {
        release {
            isMinifyEnabled = false
            proguardFiles(
                getDefaultProguardFile("proguard-android-optimize.txt"),
                "proguard-rules.pro"
            )
        }
    }
    compileOptions {
        sourceCompatibility = JavaVersion.VERSION_11
        targetCompatibility = JavaVersion.VERSION_11
    }
    kotlinOptions {
        jvmTarget = "11"
        freeCompilerArgs += listOf("-Xjvm-default=all")
    }

    sourceSets {
        getByName("main") {
            java.srcDirs(
                "src/main/java",
                "../../../crates/lingxia-webview/src/android/java"
            )
        }
    }

    // The com.vanniktech.maven.publish plugin owns the release publication
    // (it configures the single "release" variant with sources + javadoc below
    // via mavenPublishing { configure(AndroidSingleVariantLibrary(...)) }).
}

dependencies {
    implementation(libs.androidx.core.ktx)
    implementation(libs.androidx.appcompat)
    implementation(libs.androidx.webkit)
    implementation(libs.material)
    implementation(libs.androidsvg)
    implementation(libs.androidx.viewpager2)
    implementation(libs.media3.exoplayer)
    implementation(libs.media3.exoplayer.hls)
    implementation(libs.media3.ui)
    implementation(libs.media3.transformer)
    implementation(libs.androidx.camera.core)
    implementation(libs.androidx.camera.camera2)
    implementation(libs.androidx.camera.lifecycle)
    implementation(libs.androidx.camera.view)
    implementation(libs.androidx.camera.video)
    implementation(libs.androidx.exifinterface)
    implementation(libs.mlkit.barcode.scanning)
    testImplementation(libs.junit)
    androidTestImplementation(libs.androidx.test.runner)
    androidTestImplementation(libs.androidx.test.core)
    androidTestImplementation(libs.androidx.test.ext.junit)
}

val sdkGroupId = "io.github.lingxia-dev"
val sdkArtifactId = "lingxia"
val sdkVersion = (project.findProperty("version") as String?) ?: "0.0.1"

// Publishing only applies when building the SDK standalone for release; the
// example app includes this module as a project (rootProject "lingxia-example")
// and must not apply the publishing plugin.
if (rootProject.name == "lingxia-sdk") {
    apply(plugin = "com.vanniktech.maven.publish")
    extensions.configure<com.vanniktech.maven.publish.MavenPublishBaseExtension> {
    coordinates(sdkGroupId, sdkArtifactId, sdkVersion)

    // Single-variant Android library: build the "release" variant with a
    // sources jar and a javadoc jar. The vanniktech plugin is used only to
    // assemble this publication; it is published to the local "localExample"
    // repo below (zipped into the GitHub release artifact) — never to Maven
    // Central. The lingxia CLI downloads that zip and resolves the SDK from it.
    configure(
        AndroidSingleVariantLibrary(
            variant = "release",
            sourcesJar = true,
            publishJavadocJar = true,
        )
    )
    }
}

// Keep a local-directory Maven repository so scripts/release/sdk.sh can publish
// the AAR + POM to a workspace dir (and zip it as a release artifact) without
// touching Maven Central. The vanniktech plugin adds the publication; this only
// adds an extra destination repository named "localExample".
publishing {
    repositories {
        maven {
            name = "localExample"
            val repoDirProp = project.findProperty("LOCAL_MAVEN_REPO_DIR") as String?
            // Default to the workspace Rust cargo target directory
            val fallback = File(rootProject.projectDir, "../../target/maven").absolutePath
            url = uri(repoDirProp ?: fallback)
        }
    }
}
