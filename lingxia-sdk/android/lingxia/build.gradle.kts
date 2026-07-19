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
    implementation("androidx.webkit:webkit:1.15.0")
    implementation(libs.material)
    implementation("androidx.viewpager2:viewpager2:1.0.0")
    implementation("androidx.media3:media3-exoplayer:1.4.1")
    implementation("androidx.media3:media3-exoplayer-hls:1.4.1")
    implementation("androidx.media3:media3-ui:1.4.1")
    implementation("androidx.media3:media3-transformer:1.4.1")
    implementation("androidx.camera:camera-core:1.3.4")
    implementation("androidx.camera:camera-camera2:1.3.4")
    implementation("androidx.camera:camera-lifecycle:1.3.4")
    implementation("androidx.camera:camera-view:1.3.4")
    implementation("androidx.camera:camera-video:1.3.4")
    implementation("androidx.exifinterface:exifinterface:1.3.6")
    implementation("com.google.mlkit:barcode-scanning:17.2.0")
    testImplementation("junit:junit:4.13.2")
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
